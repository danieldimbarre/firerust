use std::fmt::{ Display, Formatter };
use connector::{ Connector, Method };
use serde::de::DeserializeOwned;
use std::error::Error;
use serde::Serialize;
use url::Url;


pub mod connector;


#[derive(Debug, Clone)]
pub struct FirebaseClient {
    connector: Connector,
    api_key: Option<String>,
}

impl FirebaseClient {
    pub fn new(url: impl ToString) -> Result<FirebaseClient, Box<dyn Error>> {
        let url = Url::parse(&url.to_string())?;

        let domain = match url.domain() {
            Some(domain) => {
                if !domain.contains(".firebaseio.com") && !domain.contains(".firebasedatabase.app") {
                    return Err(Box::new(FirebaseError::new("Invalid domain")));
                }

                domain.to_string()
            },
            None => return Err(Box::new(FirebaseError::new("Invalid domain")))
        };

        let port = match url.port_or_known_default() {
            Some(port) => port,
            None => 443 as u16
        };


        Ok(FirebaseClient {
            api_key: None,
            connector: Connector::new(domain, port)?
        })
    }

    pub fn auth(&mut self, api_key: impl ToString) {
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

    pub fn get<T>(&self) -> Result<T, Box<dyn Error>> where T: Serialize + DeserializeOwned {
        let response = self.client.connector.request(Method::GET, self.path.clone(), match self.client.api_key {
            Some(ref api_key) => format!("?auth={}", api_key),
            None => "".to_string()
        }, None)?;

        if response.status().code() != 200 {
            return Err(Box::new(FirebaseError::new(format!("{} {}", response.status().code(), response.status().message()))));
        }

        Ok(serde_json::from_str(response.body())?)
    }

    pub fn set<T>(&self, data: T) -> Result<(), Box<dyn Error>>  where T: Serialize {
        let data = serde_json::to_string(&data)?;

        let response = self.client.connector.request(Method::PUT, self.path.clone(), match self.client.api_key {
            Some(ref api_key) => format!("??print=silent&auth={}", api_key),
            None => "?print=silent".to_string()
        }, Some(data))?;

        if response.status().code() != 204 {
            return Err(Box::new(FirebaseError::new(format!("{} {}", response.status().code(), response.status().message()))));
        }

        Ok(())
    }

    pub fn set_unique<T>(&self, data: T) -> Result<(), Box<dyn Error>>  where T: Serialize {
        let data = serde_json::to_string(&data)?;

        let response = self.client.connector.request(Method::POST, self.path.clone(), match self.client.api_key {
            Some(ref api_key) => format!("??print=silent&auth={}", api_key),
            None => "?print=silent".to_string()
        }, Some(data))?;

        if response.status().code() != 204 {
            return Err(Box::new(FirebaseError::new(format!("{} {}", response.status().code(), response.status().message()))));
        }

        Ok(())
    }

    pub fn update<T>(&self, data: T) -> Result<(), Box<dyn Error>> where T: Serialize {
        let data = serde_json::to_string(&data)?;

        let response = self.client.connector.request(Method::PATCH, self.path.clone(), match self.client.api_key {
            Some(ref api_key) => format!("??print=silent&auth={}", api_key),
            None => "?print=silent".to_string()
        }, Some(data))?;

        if response.status().code() != 204 {
            return Err(Box::new(FirebaseError::new(format!("{} {}", response.status().code(), response.status().message()))));
        }

        Ok(())
    }

    pub fn delete(&self) -> Result<(), Box<dyn Error>> {
        let response = self.client.connector.request(Method::DELETE, self.path.clone(), match self.client.api_key {
            Some(ref api_key) => format!("??print=silent&auth={}", api_key),
            None => "?print=silent".to_string()
        }, None)?;

        if response.status().code() != 204 {
            return Err(Box::new(FirebaseError::new(format!("{} {}", response.status().code(), response.status().message()))));
        }

        Ok(())
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