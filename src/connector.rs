use native_tls::{ TlsConnector, TlsStream };
use std::fmt::{ Display, Formatter };
use std::sync::{ Arc, Mutex };
use std::io::{ Write, Read };
use std::net::TcpStream;
use std::error::Error;


#[derive(Clone, Debug)]
pub struct Connector {
    host: String,
    domain: String,
    stream: Arc<Mutex<TlsStream<TcpStream>>>
}

impl Connector {
    pub fn new(domain: impl ToString, port: impl ToString) -> Result<Connector, Box<dyn Error>> {
        let tlsconnector = TlsConnector::new()?;

        let stream = match TcpStream::connect(domain.to_string() + ":" + &port.to_string()) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                tlsconnector.connect(&domain.to_string(), stream)?
            },
            Err(_) => return Err(Box::new(ConnectorError::GatewayTimeout))
        };

        Ok(Connector {
            domain: domain.to_string(),
            stream: Arc::new(Mutex::new(stream)),
            host: domain.to_string() + ":" + &port.to_string()
        })
    }

    pub fn reconnect(&self) -> Result<(), Box<dyn Error>> {
        let tlsconnector = TlsConnector::new()?;

        let stream = match TcpStream::connect(self.host.clone()) {
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
    
    pub fn request(&self, method: Method, path: String, params: String, data: Option<String>) -> Result<Response, Box<dyn Error>> {
        let mut stream = match self.stream.lock() {
            Ok(stream) => stream,
            Err(_) => return Err(Box::new(ConnectorError::GatewayTimeout))
        };

        stream.write_all(format!("{} {}.json{} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nKeep-Alive: timeout=5, max=100\r\nAccept: application/json; charset=utf-8\r\nCache-Control: no-cache{}", method.to_string(), path, params, self.domain, match data {
            Some(data) => format!("\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{}", data.as_bytes().len(), data),
            None => String::from("\r\n\r\n")
        }).as_bytes())?;
        
        let mut data = Vec::new();

        loop {
            let buffer = &mut [0; 1024];
            let size = stream.read(buffer)?;

            data.extend_from_slice(&buffer[0..size]);

            if size < 1024 {
                break;
            }
        }

        let response = String::from_utf8(data)?;
        let mut response = response.split("\r\n\r\n");

        let mut header = match response.next() {
            Some(header) => header.lines(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let body = match response.next() {
            Some(body) => body,
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_line = match header.next() {
            Some(status_line) => status_line.split(" ").collect::<Vec<&str>>(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_code = match status_line.get(1) {
            Some(status_code) => match status_code.parse::<u16>() {
                Ok(status_code) => status_code,
                Err(_) => return Err(Box::new(ConnectorError::InvalidResponse))
            },
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_message = match status_line.get(2) {
            Some(status_message) => status_message.to_string(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        Ok(Response::new(body, Status::new(status_code, status_message)))
    }

    pub fn event_stream(&self, path: String, params: String) -> Result<(Status, EventStream, TlsStream<TcpStream>), Box<dyn Error>> {
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

        let mut data = Vec::new();

        loop {
            let buffer = &mut [0; 2048];
            let size = stream.read(buffer)?;

            data.extend_from_slice(&buffer[0..size]);

            if size < 1024 {
                break;
            }
        }

        let _tmp = String::from_utf8(data)?;
        let mut response = _tmp.split("\r\n\r\n");

        let mut header = match response.next() {
            Some(header) => header.lines(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };
        
        let mut body = match response.next() {
            Some(body) => body.to_string(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_line = match header.next() {
            Some(status_line) => status_line.split(" ").collect::<Vec<&str>>(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_code = match status_line.get(1) {
            Some(status_code) => match status_code.parse::<u16>() {
                Ok(status_code) => status_code,
                Err(_) => return Err(Box::new(ConnectorError::InvalidResponse))
            },
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        let status_message = match status_line.get(2) {
            Some(status_message) => status_message.to_string(),
            None => return Err(Box::new(ConnectorError::InvalidResponse))
        };

        if body == "" {
            loop {
                let buffer = &mut [0; 2048];
                let size = stream.read(buffer)?;
                let data = match String::from_utf8(buffer[0..size].to_vec()) {
                    Ok(data) => data,
                    Err(_) => return Err(Box::new(ConnectorError::GatewayTimeout))
                };

                body.push_str(&data);
    
                if size < 1024 {
                    break;
                }
            }
        }

        Ok((Status::new(status_code, status_message), EventStream::try_from(body.to_string())?, stream))
    }
}


pub struct Status {
    code: u16,
    message: String
}

impl Status {
    pub fn new(code: u16, message: impl ToString) -> Status {
        Status {
            code: code,
            message: message.to_string()
        }
    }

    pub fn code(&self) -> u16 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{} {}", self.code(), self.message())
    }
}


pub struct Response {
    body: String,
    status: Status,
}

impl Response {
    pub fn new(body: impl ToString, status: Status) -> Response {
        Response {
            body: body.to_string(),
            status: status
        }
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn status(&self) -> &Status {
        &self.status
    }
}


pub enum EventType {
    Put,
    Patch,
    Cancel,
    KeepAlive,
    AuthRevoked
}

impl From<String> for EventType {
    fn from(event: String) -> EventType {
        match event.as_str() {
            "put" => EventType::Put,
            "patch" => EventType::Patch,
            "cancel" => EventType::Cancel,
            "keep-alive" => EventType::KeepAlive,
            "auth_revoked" => EventType::AuthRevoked,
            _ => EventType::Put
        }
    }
}


pub struct EventStream {
    event: EventType,
    data: String,
}

impl EventStream {
    pub fn new(event: impl ToString, data: impl ToString) -> EventStream {
        EventStream {
            data: data.to_string(),
            event: EventType::from(event.to_string())
        }
    }

    pub fn event(&self) -> &EventType {
        &self.event
    }

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


pub enum Method {
    Get,
    Put,
    Post,
    Patch,
    Delete
}

impl ToString for Method {
    fn to_string(&self) -> String {
        match self {
            Method::Get => "GET",
            Method::Put => "PUT",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE"
        }.to_string()
    }
}


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

impl Error for ConnectorError {
    fn description(&self) -> &str {
        match self {
            ConnectorError::LockError => "Lock error",
            ConnectorError::GatewayTimeout => "Gateway Timeout",
            ConnectorError::InvalidResponse => "Invalid response",
        }
    }
}