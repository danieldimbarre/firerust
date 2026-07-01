//! # Example
//! 
//! ```rust,no_run
//! use firerust::connector::{Connector, Method};
//! 
//! #[tokio::main]
//! async fn main() {
//!     let connector = Connector::new("docs-example.firebaseio.com", 443).unwrap();
//!     let response = connector.request(Method::Get, "/", None, None, None).await.unwrap();
//! }
//! ```


use reqwest::{Client, ClientBuilder, Response as ReqwestResponse};
use std::fmt::{ Display, Formatter };
use std::error::Error;

/// A connector to a Firebase server.
#[derive(Clone, Debug)]
pub struct Connector {
    client: Client,
    base_url: String,
}

impl Connector {

    /// Creates a new connector
    pub fn new(domain: impl ToString, port: u16) -> Result<Connector, ConnectorError> {
        let mut domain_str = domain.to_string();
        if domain_str.ends_with('/') {
            domain_str.pop();
        }
        
        let base_url = format!("https://{}:{}", domain_str, port);

        // Firebase has high keep-alive limits, reqwest pools automatically
        let client = ClientBuilder::new()
            .tcp_nodelay(true)
            .build()
            .map_err(|e| ConnectorError::Reqwest(e))?;

        Ok(Connector {
            client,
            base_url
        })
    }

    fn build_url(&self, path: &str, params: Option<&str>) -> String {
        let mut p = path;
        if p.starts_with('/') {
            p = &p[1..];
        }
        if p.ends_with('/') {
            p = &p[..p.len()-1];
        }
        
        let params_str = params.unwrap_or("");
        
        format!("{}/{}.json{}", self.base_url, p, params_str)
    }

    /// Send data to the server
    pub async fn request(&self, method: Method, path: &str, params: Option<&str>, data: Option<&str>, api_key: Option<&str>) -> Result<Response, ConnectorError> {
        let url = self.build_url(path, params);
        
        let mut builder = match method {
            Method::Get => self.client.get(&url),
            Method::Put => self.client.put(&url),
            Method::Post => self.client.post(&url),
            Method::Patch => self.client.patch(&url),
            Method::Delete => self.client.delete(&url),
        };

        if let Some(key) = api_key {
            builder = builder.bearer_auth(key);
        }

        if let Some(body_data) = data {
            builder = builder.header("Content-Type", "application/json")
                             .body(body_data.to_string());
        }

        let res = builder.send().await?;
        let status_code = res.status().as_u16();
        let status_msg = res.status().canonical_reason().unwrap_or("Unknown").to_string();
        let body = res.text().await?;

        Ok(Response::new(body, Status::new(status_code, status_msg)))
    }

    /// Connect to the server with event stream
    pub async fn event_stream(&self, path: &str, params: Option<&str>, api_key: Option<&str>) -> Result<ReqwestResponse, ConnectorError> {
        let url = self.build_url(path, params);
        
        let mut builder = self.client.get(&url).header("Accept", "text/event-stream");

        if let Some(key) = api_key {
            builder = builder.bearer_auth(key);
        }

        let res = builder.send().await?;
        Ok(res)
    }
}


/// Status response
pub struct Status {
    code: u16,
    message: String
}

impl Status {

    /// Create a new status
    pub fn new(code: u16, message: impl ToString) -> Status {
        Status {
            code: code,
            message: message.to_string()
        }
    }

    /// Get the status code
    pub fn code(&self) -> u16 {
        self.code
    }

    /// Get the status message
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{} {}", self.code(), self.message())
    }
}


/// Database request response
pub struct Response {
    body: String,
    status: Status,
}

impl Response {
    
    /// Create a new response
    pub fn new(body: impl ToString, status: Status) -> Response {
        Response {
            body: body.to_string(),
            status: status
        }
    }

    /// Get the response body
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Get the response status
    pub fn status(&self) -> &Status {
        &self.status
    }
}


/// Types of stream events
#[derive(Debug)]
pub enum EventType {
    Put,
    Patch,
    Cancel,
    KeepAlive,
    AuthRevoked,
    Unknown(String)
}

impl From<String> for EventType {
    fn from(event: String) -> EventType {
        match event.as_str() {
            "put" => EventType::Put,
            "patch" => EventType::Patch,
            "cancel" => EventType::Cancel,
            "keep-alive" => EventType::KeepAlive,
            "auth_revoked" => EventType::AuthRevoked,
            _ => EventType::Unknown(event)
        }
    }
}


/// Event stream parser
#[derive(Debug)]
pub struct EventStream {
    event: EventType,
    data: String,
}

impl EventStream {
    
    /// Create a new event stream
    pub fn new(event: impl ToString, data: impl ToString) -> EventStream {
        EventStream {
            data: data.to_string(),
            event: EventType::from(event.to_string())
        }
    }

    /// Get the event type
    pub fn event(&self) -> &EventType {
        &self.event
    }

    /// Get the event data
    pub fn data(&self) -> &str {
        &self.data
    }
}

impl TryFrom<String> for EventStream {
    type Error = &'static str;

    fn try_from(data: String) -> Result<Self, Self::Error> {
        let mut event_stream = data.lines().filter(|l| l.contains(":"));

        let event = match event_stream.next() {
            Some(event) => match event.strip_prefix("event: ") {
                Some(event) => event.to_string(),
                None => return Err("Invalid event")
            },
            None => return Err("Invalid event stream")
        };

        let data = match event_stream.next() {
            Some(data) => match data.strip_prefix("data: ") {
                Some(data) => data.to_string(),
                None => return Err("Invalid data")
            },
            None => return Err("Invalid event stream")
        };

        Ok(EventStream::new(event, data))
    }
}


#[derive(Clone, Debug)]
pub enum Method {
    Get,
    Put,
    Post,
    Patch,
    Delete
}

impl Display for Method {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Method::Get => "GET",
            Method::Put => "PUT",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE"
        };
        write!(f, "{}", s)
    }
}


/// Errors that can occur when getting a database response
#[derive(Debug)]
pub enum ConnectorError {
    Reqwest(reqwest::Error),
    EventParse(String),
}

impl Display for ConnectorError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ConnectorError::Reqwest(e) => write!(f, "Request error: {}", e),
            ConnectorError::EventParse(e) => write!(f, "Event parse error: {}", e),
        }
    }
}

impl Error for ConnectorError {}

impl From<reqwest::Error> for ConnectorError { fn from(e: reqwest::Error) -> Self { ConnectorError::Reqwest(e) } }
impl From<std::string::FromUtf8Error> for ConnectorError { fn from(e: std::string::FromUtf8Error) -> Self { ConnectorError::EventParse(e.to_string()) } }
impl From<&'static str> for ConnectorError { fn from(e: &'static str) -> Self { ConnectorError::EventParse(e.to_string()) } }
