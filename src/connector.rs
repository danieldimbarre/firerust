//! # Example
//! 
//! ```rust,no_run
//! use firerust::connector::Connector;
//! 
/// let connector = Connector::new("docs-example.firebaseio.com", 443)?;
/// let response = connector.request(Method::Get, "/", None, None)?;
/// 
/// drop(connector);
/// ```


use native_tls::{ TlsConnector, TlsStream };
use url::Url;
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
    pub fn new(domain: impl ToString, port: u16) -> Result<Connector, ConnectorError> {
        let domain_str = domain.to_string();
        let tlsconnector = TlsConnector::new()?;
        let host = format!("{}:{}", domain_str, port);

        let stream = match TcpStream::connect(&host) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                tlsconnector.connect(&domain_str, stream)?
            },
            Err(_) => return Err(ConnectorError::GatewayTimeout)
        };

        Ok(Connector {
            domain: domain_str,
            stream: Arc::new(Mutex::new(stream)),
            host
        })
    }

    /// Reconnect the stream
    pub fn reconnect(&self) -> Result<(), ConnectorError> {
        let tlsconnector = TlsConnector::new()?;

        let stream = match TcpStream::connect(&self.host) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                tlsconnector.connect(&self.domain, stream)?
            },
            Err(_) => return Err(ConnectorError::GatewayTimeout)
        };

        match self.stream.lock() {
            Ok(mut old_stream) => {
                *old_stream = stream;
            },
            Err(_) => return Err(ConnectorError::LockError)
        };
        
        Ok(())
    }
    
    /// Send data to the server (with auto-reconnect on IO error)
    pub fn request(&self, method: Method, path: &str, params: Option<&str>, data: Option<&str>, api_key: Option<&str>) -> Result<Response, ConnectorError> {
        let mut retries = 1;
        loop {
            match self.request_inner(method.clone(), path, params, data, api_key) {
                Ok(res) => return Ok(res),
                Err(e) => {
                    if let ConnectorError::Io(_) = e {
                        if retries > 0 {
                            retries -= 1;
                            let _ = self.reconnect();
                            continue;
                        }
                    }
                    return Err(e);
                }
            }
        }
    }

    fn request_inner(&self, method: Method, path: &str, params: Option<&str>, data: Option<&str>, api_key: Option<&str>) -> Result<Response, ConnectorError> {
        let mut stream = match self.stream.lock() {
            Ok(stream) => stream,
            Err(_) => return Err(ConnectorError::GatewayTimeout)
        };

        let auth_header = match api_key {
            Some(key) => format!("\r\nAuthorization: Bearer {}", key),
            None => String::new()
        };

        let params_str = params.unwrap_or("");
        let data_header = match data {
            Some(d) => format!("\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", d.len(), d),
            None => String::from("\r\n\r\n")
        };

        let req = format!("{} {}.json{} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nKeep-Alive: timeout=5, max=100\r\nAccept: application/json; charset=utf-8\r\nCache-Control: no-cache{}{}", method, path, params_str, self.domain, auth_header, data_header);
        stream.write_all(req.as_bytes())?;
        
        let mut headers_data = Vec::new();
        let mut headers_end = 0;

        loop {
            let buffer = &mut [0; 8192];
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
            return Err(ConnectorError::InvalidResponse);
        }

        let header_str = String::from_utf8_lossy(&headers_data[..headers_end]).to_string();
        let mut content_length = 0;
        let mut is_chunked = false;

        let mut header_lines = header_str.lines();
        let status_line = match header_lines.next() {
            Some(line) => line.split(' ').collect::<Vec<&str>>(),
            None => return Err(ConnectorError::InvalidResponse)
        };

        let status_code = status_line.get(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        let status_message = if status_line.len() > 2 { status_line[2..].join(" ") } else { String::new() };

        for line in header_lines {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse::<usize>().unwrap_or(0);
                }
            } else if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
                is_chunked = true;
            }
        }

        let mut body_data = headers_data[headers_end..].to_vec();

        if is_chunked {
            let mut final_body = Vec::new();
            loop {
                while !body_data.windows(2).any(|w| w == b"\r\n") {
                    let buffer = &mut [0; 8192];
                    let size = stream.read(buffer)?;
                    if size == 0 { break; }
                    body_data.extend_from_slice(&buffer[0..size]);
                }
                
                let pos = match body_data.windows(2).position(|w| w == b"\r\n") {
                    Some(p) => p,
                    None => break
                };
                
                let chunk_size_str = String::from_utf8_lossy(&body_data[..pos]).trim().to_string();
                let chunk_size = usize::from_str_radix(&chunk_size_str, 16).unwrap_or(0);
                
                body_data = body_data[pos+2..].to_vec();
                
                if chunk_size == 0 {
                    break;
                }
                
                while body_data.len() < chunk_size + 2 {
                    let buffer = &mut [0; 8192];
                    let size = stream.read(buffer)?;
                    if size == 0 { break; }
                    body_data.extend_from_slice(&buffer[0..size]);
                }
                
                final_body.extend_from_slice(&body_data[..chunk_size]);
                body_data = body_data[chunk_size+2..].to_vec();
            }
            body_data = final_body;
        } else {
            while body_data.len() < content_length {
                let buffer = &mut [0; 8192];
                let size = stream.read(buffer)?;
                if size == 0 {
                    break;
                }
                body_data.extend_from_slice(&buffer[0..size]);
            }
        }

        let body = String::from_utf8_lossy(&body_data).to_string();
        Ok(Response::new(body, Status::new(status_code, status_message)))
    }

    /// Connect to the server with event stream
    pub fn event_stream(&self, path: &str, params: Option<&str>, api_key: Option<&str>) -> Result<(Status, EventStream, TlsStream<TcpStream>, Vec<u8>), ConnectorError> {
        let tlsconnector = TlsConnector::new()?;

        let mut stream = match TcpStream::connect(&self.host) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                tlsconnector.connect(&self.domain, stream)?
            },
            Err(_) => return Err(ConnectorError::GatewayTimeout)
        };

        let auth_header = match api_key {
            Some(key) => format!("\r\nAuthorization: Bearer {}", key),
            None => String::new()
        };
        let params_str = params.unwrap_or("");

        let req = format!("GET {}.json{} HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream{}\r\n\r\n", path, params_str, self.domain, auth_header);
        stream.write_all(req.as_bytes())?;
        stream.flush()?;

        let mut headers_data = Vec::new();
        let mut headers_end = 0;

        loop {
            let buffer = &mut [0; 8192];
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
            return Err(ConnectorError::InvalidResponse);
        }

        let header_str = String::from_utf8_lossy(&headers_data[..headers_end]).to_string();
        let mut header_lines = header_str.lines();
        
        let status_line = match header_lines.next() {
            Some(line) => line.split(' ').collect::<Vec<&str>>(),
            None => return Err(ConnectorError::InvalidResponse)
        };

        let status_code = status_line.get(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        let status_message = if status_line.len() > 2 { status_line[2..].join(" ") } else { String::new() };

        if status_code == 307 || status_code == 301 || status_code == 302 {
            let location = header_lines.find(|l| l.to_lowercase().starts_with("location:"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim());
            
            if let Some(loc) = location {
                if let Ok(parsed_url) = Url::parse(loc) {
                    if let Some(domain) = parsed_url.host_str() {
                        let port = parsed_url.port_or_known_default().unwrap_or(443);
                        let host = format!("{}:{}", domain, port);
                        let mut stream = match TcpStream::connect(&host) {
                            Ok(stream) => {
                                stream.set_nodelay(true)?;
                                tlsconnector.connect(domain, stream)?
                            },
                            Err(_) => return Err(ConnectorError::GatewayTimeout)
                        };

                        let new_path = parsed_url.path();
                        let new_query = parsed_url.query().map(|q| format!("?{}", q)).unwrap_or_default();
                        
                        let req = format!("GET {}{} HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream{}\r\n\r\n", new_path, new_query, domain, auth_header);
                        stream.write_all(req.as_bytes())?;
                        stream.flush()?;
                        
                        // Recurse or parse headers again. For simplicity, just return a recursive call on a new temporary Connector.
                        let temp_connector = Connector::new(domain, port)?;
                        // Wait, our new event_stream signature takes path and params separately. We can just use the url path and query!
                        let stripped_path = new_path.strip_suffix(".json").unwrap_or(new_path);
                        return temp_connector.event_stream(stripped_path, if new_query.is_empty() { None } else { Some(&new_query) }, api_key);
                    }
                }
            }
        }

        let mut body_data = headers_data[headers_end..].to_vec();
        while !body_data.windows(2).any(|w| w == b"\n\n") {
            let buffer = &mut [0; 8192];
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
    LockError,
    GatewayTimeout,
    InvalidResponse,
    Io(std::io::Error),
    Tls(String),
    EventParse(String),
}

impl Display for ConnectorError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ConnectorError::LockError => write!(f, "Lock error"),
            ConnectorError::GatewayTimeout => write!(f, "Gateway Timeout"),
            ConnectorError::InvalidResponse => write!(f, "Invalid response"),
            ConnectorError::Io(e) => write!(f, "IO error: {}", e),
            ConnectorError::Tls(e) => write!(f, "TLS error: {}", e),
            ConnectorError::EventParse(e) => write!(f, "Event parse error: {}", e),
        }
    }
}


impl Error for ConnectorError {}

impl From<std::io::Error> for ConnectorError { fn from(e: std::io::Error) -> Self { ConnectorError::Io(e) } }
impl From<native_tls::Error> for ConnectorError { fn from(e: native_tls::Error) -> Self { ConnectorError::Tls(e.to_string()) } }
impl From<native_tls::HandshakeError<std::net::TcpStream>> for ConnectorError { fn from(e: native_tls::HandshakeError<std::net::TcpStream>) -> Self { ConnectorError::Tls(e.to_string()) } }
impl From<std::string::FromUtf8Error> for ConnectorError { fn from(e: std::string::FromUtf8Error) -> Self { ConnectorError::EventParse(e.to_string()) } }
impl From<&'static str> for ConnectorError { fn from(e: &'static str) -> Self { ConnectorError::EventParse(e.to_string()) } }
