use std::fmt::{ Display, Formatter, Debug };
use serde::de::DeserializeOwned;
use native_tls::TlsConnector;
use std::io::{ Write, Read };
use std::net::TcpStream;
use std::error::Error;
use serde::Serialize;
use url::Url;


#[derive(Debug, Clone)]
pub struct FirebaseClient {
    url: Url,
    connector: TlsConnector,
    api_key: Option<String>,
}

impl FirebaseClient {
    pub fn new(url: impl ToString) -> Result<FirebaseClient, Box<dyn Error>> {
        let url = Url::parse(&url.to_string())?;

        if let Some(domain) = url.domain() {
            if !domain.contains(".firebaseio.com") && !domain.contains(".firebasedatabase.app") {
                return Err(Box::new(FirebaseError::new("Invalid domain")));
            }
        } else {
            return Err(Box::new(FirebaseError::new("Invalid domain")));
        };

        Ok(FirebaseClient {
            url,
            api_key: None,
            connector: TlsConnector::new()?,
        })
    }

    pub fn auth(&mut self, api_key: &str) {
        self.api_key = Some(api_key.to_string());
    }

    pub fn reference(&self, path: impl ToString) -> RealtimeReference {
        RealtimeReference::new(self, path.to_string())
    }
}

pub struct RealtimeReference {
    client: FirebaseClient,
    path: String,
}

impl RealtimeReference {
    pub fn new(client: &FirebaseClient, path: impl ToString) -> RealtimeReference {
        RealtimeReference {
            client: client.clone(),
            path: path.to_string(),
        }
    }

    pub fn child(&self, path: impl ToString) -> RealtimeReference {
        RealtimeReference::new(&self.client, format!("{}/{}", self.path, path.to_string()))
    }

    pub fn get<T>(&self) -> Result<T, Box<dyn Error>> where T: Serialize + DeserializeOwned + Debug {
        let host = match self.client.url.domain() {
            Some(host) => host,
            None => return Err(Box::new(FirebaseError::new("Invalid URL")))
        };

        let port = match self.client.url.port_or_known_default() {
            Some(port) => port,
            None => return Err(Box::new(FirebaseError::new("Invalid URL")))
        };

        let mut buf = String::new();
        let stream = TcpStream::connect(format!("{}:{}", host, port))?;
        let mut stream = self.client.connector.connect(host, stream)?;

        stream.write_all(format!("GET {}.json HTTP/1.0\r\nHost: {}\r\nAccept: application/json; charset=utf-8\r\nCache-Control: no-cache\r\n\r\n", self.path, host).as_bytes())?;
        stream.read_to_string(&mut buf)?;

        let mut response = buf.split("\r\n\r\n");
        
        let mut header = match response.next() {
            Some(header) => header.lines(),
            None => return Err(Box::new(FirebaseError::new("Invalid response")))
        };

        let body = match response.next() {
            Some(body) => body,
            None => return Err(Box::new(FirebaseError::new("Invalid response")))
        };

        let status = match header.next() {
            Some(status) => status.split(" ").collect::<Vec<&str>>(),
            None => return Err(Box::new(FirebaseError::new("Invalid response")))
        };

        if status[1] != "200" {
            return Err(Box::new(FirebaseError::new(format!("{} {}", status[1], status[2]))));
        }

        Ok(serde_json::from_str(body)?)
    }
}

#[derive(Debug)]
struct FirebaseError {
    message: String
}

impl FirebaseError {
    fn new(message: impl ToString) -> FirebaseError {
        FirebaseError {
            message: message.to_string()
        }
    }
}

impl Error for FirebaseError {
    fn description(&self) -> &str {
        &self.message
    }
}

impl Display for FirebaseError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}