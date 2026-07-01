//! # Example
//! 
//! ```rust
//! use firerust::connector::Connector;
//! 
//! let connector = Connector::new("docs-example.firebaseio.com", 443)?;
//! let response = connector.request(Method::Get, "/", None, None)?;
//! 
//! drop(connector);
//! ```


use native_tls::{ TlsConnector, TlsStream };
use std::fmt::{ Display, Formatter };
use std::sync::{ Arc, Mutex };
use std::io::{ Write, Read };
use std::net::TcpStream;
use std::error::Error;


/// A connector to a Firebase server.
#[derive(Clone, Debug)]
pub struct Connector {
    host: String,
    domain: String,
    stream: Arc<Mutex<TlsStream<TcpStream>>>
}

impl Connector {

    /// Creates a new connector
    pub fn new(domain: impl ToString, port: u16) -> Result<Connector, Box<dyn Error>> {
        let domain_str = domain.to_string();
        let tlsconnector = TlsConnector::new()?;
        let host = format!("{}:{}", domain_str, port);

        let stream = match TcpStream::connect(&host) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                tlsconnector.connect(&domain_str, stream)?
            },
            Err(_) => return Err(Box::new(ConnectorError::GatewayTimeout))
        };

        Ok(Connector {
            domain: domain_str,
            stream: Arc::new(Mutex::new(stream)),
            host
        })
    }

    /// Reconnect to the server
    /// 
    /// # Example
    /// ```rust
    /// use firerust::connector::Connector;
    /// 
    /// let connector = Connector::new("docs-example.firebaseio.com", 443)?;
    /// connector.reconnect()?;
    /// ```
    pub fn reconnect(&self) -> Result<(), Box<dyn Error>> {
        let tlsconnector = TlsConnector::new()?;

        let stream = match TcpStream::connect(&self.host) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                tlsconnector.connect(&self.domain, stream)?
            },
            Err(_) => return Err(Box::new(ConnectorError::GatewayTimeout))
        };

        match self.stream.lock() {
            Ok(mut old_stream) => {
                *old_stream = stream;
            },
            Err(_) => return Err(Box::new(ConnectorError::LockError))
        };
        
        Ok(())
    }
    
    /// Send data to the server
    /// 
    /// # Example
    /// ```rust
    /// use firerust::connector::{ Connector, Method };
    /// 
    /// let connector = Connector::new("docs-example.firebaseio.com", 443)?;
    /// connector.request(Method::Get, "/", None, None)?;
    /// ```
    pub fn request(&self, method: Method, path: impl ToString, params: Option<String>, data: Option<String>) -> Result<Response, Box<dyn Error>> {
        let mut stream = match self.stream.lock() {
            Ok(stream) => stream,
            Err(_) => return Err(Box::new(ConnectorError::GatewayTimeout))
        };

        stream.write_all(format!("{} {}.json{} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nKeep-Alive: timeout=5, max=100\r\nAccept: application/json; charset=utf-8\r\nCache-Control: no-cache{}", method.to_string(), path.to_string(), match params {
            Some(params) => params,
            None => String::from("")
        }, self.domain, match data {
            Some(data) => format!("\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", data.as_bytes().len(), data),
            None => String::from("\r\n\r\n")
        }).as_bytes())?;
        
        let mut headers_data = Vec::new();
        let mut headers_end = 0;

        loop {
            let buffer = &mut [0; 1024];
            let size = stream.read(buffer)?;
            if size == 0 {
                break;
            }
            headers_data.extend_from_slice(&buffer[0..size]);

            if let Some(pos) = headers_data.windows(4).position(|w| w == b"\r\n\r\n") {
                headers_end = pos + 4;
                break;
            }
        }

        if headers_end == 0 {
            return Err(Box::new(ConnectorError::InvalidResponse));
        }

        let header_str = String::from_utf8_lossy(&headers_data[..headers_end]).to_string();
        let mut content_length = 0;

        let mut header_lines = header_str.lines();
        let status_line = match header_lines.next() {
            Some(line) => line.split(' ').collect::<Vec<&str>>(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_code = status_line.get(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        let status_message = if status_line.len() > 2 { status_line[2..].join(" ") } else { String::new() };

        for line in header_lines {
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse::<usize>().unwrap_or(0);
                }
            }
        }

        let mut body_data = headers_data[headers_end..].to_vec();
        while body_data.len() < content_length {
            let buffer = &mut [0; 1024];
            let size = stream.read(buffer)?;
            if size == 0 {
                break;
            }
            body_data.extend_from_slice(&buffer[0..size]);
        }

        let body = String::from_utf8(body_data)?;
        Ok(Response::new(body, Status::new(status_code, status_message)))
    }

    /// Connect to the server with event stream
    pub fn event_stream(&self, path: String, params: String) -> Result<(Status, EventStream, TlsStream<TcpStream>, Vec<u8>), Box<dyn Error>> {
        let tlsconnector = TlsConnector::new()?;

        let mut stream = match TcpStream::connect(self.host.clone()) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                tlsconnector.connect(&self.domain, stream)?
            },
            Err(_) => return Err(Box::new(ConnectorError::GatewayTimeout))
        };

        stream.write_all(format!("GET {}.json{} HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream\r\n\r\n", path, params, self.domain).as_bytes())?;
        stream.flush()?;

        let mut headers_data = Vec::new();
        let mut headers_end = 0;

        loop {
            let buffer = &mut [0; 1024];
            let size = stream.read(buffer)?;
            if size == 0 {
                break;
            }
            headers_data.extend_from_slice(&buffer[0..size]);

            if let Some(pos) = headers_data.windows(4).position(|w| w == b"\r\n\r\n") {
                headers_end = pos + 4;
                break;
            }
        }

        if headers_end == 0 {
            return Err(Box::new(ConnectorError::InvalidResponse));
        }

        let header_str = String::from_utf8_lossy(&headers_data[..headers_end]).to_string();

        let mut header_lines = header_str.lines();
        let status_line = match header_lines.next() {
            Some(line) => line.split(' ').collect::<Vec<&str>>(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_code = status_line.get(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        let status_message = if status_line.len() > 2 { status_line[2..].join(" ") } else { String::new() };

        let mut body_data = headers_data[headers_end..].to_vec();
        while !body_data.windows(2).any(|w| w == b"\n\n") {
            let buffer = &mut [0; 1024];
            let size = stream.read(buffer)?;
            if size == 0 { break; }
            body_data.extend_from_slice(&buffer[0..size]);
        }

        let pos = body_data.windows(2).position(|w| w == b"\n\n").unwrap_or(body_data.len());
        let body_str = String::from_utf8_lossy(&body_data[..pos]).to_string();
        let remaining = if pos + 2 < body_data.len() {
            body_data[pos+2..].to_vec()
        } else {
            Vec::new()
        };

        Ok((Status::new(status_code, status_message), EventStream::try_from(body_str)?, stream, remaining))
    }
}

impl Drop for Connector {
    fn drop(&mut self) {
        if let Ok(mut stream) = self.stream.lock() {
            stream.shutdown().ok();
        }
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


/// Database request methods
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
    LockError,
    GatewayTimeout,
    InvalidResponse,
}

impl Display for ConnectorError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ConnectorError::LockError => write!(f, "Lock error"),
            ConnectorError::GatewayTimeout => write!(f, "Gateway Timeout"),
            ConnectorError::InvalidResponse => write!(f, "Invalid response"),
        }
    }
}

impl Error for ConnectorError {}