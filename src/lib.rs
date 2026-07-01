//! A very simple library to implement the Firebase real-time database in your code with the best performance
//! 
//! # Instalation
//! Add this to your `Cargo.toml`:
//! ```toml
//! [dependencies]
//! firerust = { version = "1.0.0" }
//! ```
//! 
//! # Examples
//! A basic example of data fetch:
//! ```rust,no_run
//! use firerust::{FirebaseClient, FirebaseError};
//! use serde_json::Value;
//!
//! fn main() -> Result<(), FirebaseError> {
//!     let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
//!     let reference = client.reference("/");
//! 
//!     reference.set(serde_json::json!({
//!         "message": "Hello, world!",
//!     }))?;
//!     println!("{:?}", reference.get::<Value>());
//! 
//!     Ok(())
//! }
//! ```


use connector::{ Connector, Method, EventStream , EventType };
use std::fmt::{ Display, Formatter };
use serde::de::DeserializeOwned;
use std::sync::{ Arc, Mutex };
use std::thread::JoinHandle;
use std::error::Error;
use serde_json::Value;
use serde::Serialize;
use std::io::Read;
use url::Url;


/// TLS Connector for Firebase client
pub mod connector;


/// Connects and authenticates client to Firebase
#[derive(Clone)]
pub struct FirebaseClient {
    connector: Connector,
    api_key: Option<String>,
}


impl std::fmt::Debug for FirebaseClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FirebaseClient")
            .field("connector", &self.connector)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl FirebaseClient {

    /// Creates a new instance of FirebaseClient with the given url
    /// and connects to the Firebase server
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
/// # Ok(())
/// # }
    /// ```
    /// 
    /// # Errors
    /// Returns an error if the url is invalid or the connection to the server fails
    pub fn new(url: impl ToString) -> Result<FirebaseClient, FirebaseError> {
        let url = Url::parse(&url.to_string())?;

        let domain = match url.domain() {
            Some(domain) => {
                if !domain.ends_with(".firebaseio.com") && !domain.ends_with(".firebasedatabase.app") {
                    return Err(FirebaseError::new("Invalid domain"));
                }

                domain.to_string()
            },
            None => return Err(FirebaseError::new("Invalid domain"))
        };

        let port = match url.port_or_known_default() {
            Some(port) => port,
            None => 443
        };


        Ok(FirebaseClient {
            api_key: None,
            connector: Connector::new(domain, port)?
        })
    }

    /// Sets the API key for the client
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let mut client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// client.auth("ID_TOKEN");
/// # Ok(())
/// # }
    /// ```
    pub fn auth(&mut self, api_key: impl ToString) {
        self.api_key = Some(api_key.to_string());
    }

    /// Creates a new reference to the given path
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// let reference = client.reference("/");
/// # Ok(())
/// # }
    /// ```
    pub fn reference(&self, path: impl ToString) -> RealtimeReference<'_> {
        RealtimeReference::new(self, path.to_string())
    }
}


/// A reference to a Firebase real-time database
pub struct RealtimeReference<'a> {
    client: &'a FirebaseClient,
    path: String,
}

impl<'a> RealtimeReference<'a> {

    fn write_request(&self, method: Method, data: Option<&str>) -> Result<Option<String>, FirebaseError> {
        let params = "?print=silent";
        
        let response = self.client.connector.request(
            method,
            &self.path,
            Some(params),
            data,
            self.client.api_key.as_deref()
        )?;

        let code = response.status().code();
        if code != 200 && code != 204 {
            return Err(FirebaseError::new(format!("{} {}", code, response.status().message())));
        }

        Ok(Some(response.body().to_string()))
    }

    /// Creates a new instance of RealtimeReference with the given path
    pub fn new(client: &'a FirebaseClient, path: impl ToString) -> RealtimeReference<'a> {
        RealtimeReference {
            client,
            path: path.to_string(),
        }
    }

    /// Set reference from the child path
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// let reference = client.reference("/");
    /// let child_reference = reference.child("child");
/// # Ok(())
/// # }
    /// ```
    pub fn child(&self, path: &str) -> RealtimeReference<'a> {
        RealtimeReference::new(self.client, format!("{}/{}", self.path, path))
    }

    /// Get the value of the reference
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// use serde_json::Value;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// assert_eq!(client.reference("/").get::<Value>().is_ok(), true);
/// # Ok(())
/// # }
    /// ```
    /// 
    /// # Errors
    /// Returns an error if the value is not a valid Response
    pub fn get<T>(&self) -> Result<T, FirebaseError> where T: DeserializeOwned {
        let response = self.client.connector.request(Method::Get, &self.path, None, None, self.client.api_key.as_deref())?;

        if response.status().code() != 200 {
            return Err(FirebaseError::new(format!("{} {}", response.status().code(), response.status().message())));
        }

        Ok(serde_json::from_str(response.body())?)
    }

    /// Set the value of the reference
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// client.reference("/").set(serde_json::json!({
    ///    "message": "Hello, world!",
    /// }))?;
/// # Ok(())
/// # }
    /// ```
    pub fn set<T>(&self, data: T) -> Result<(), FirebaseError> where T: Serialize {
        let data = serde_json::to_string(&data)?;
        self.write_request(Method::Put, Some(&data))?;
        Ok(())
    }

    /// Set a unique child value of the reference
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// client.reference("/posts").set_unique(serde_json::json!({
    ///     "message": "Hello, world!",
    /// }))?;
/// # Ok(())
/// # }
    /// ```
    pub fn set_unique<T>(&self, data: T) -> Result<String, FirebaseError> where T: Serialize {
        let data = serde_json::to_string(&data)?;
        let res = self.write_request(Method::Post, Some(&data))?.unwrap_or_default();
        let value: serde_json::Value = serde_json::from_str(&res)?;
        if let Some(name) = value.get("name") {
            if let Some(name_str) = name.as_str() {
                return Ok(name_str.to_string());
            }
        }
        Ok(String::new())
    }

    /// Update the value of the reference
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// client.reference("/").update(serde_json::json!({
    ///     "message": "New hello, world!",
    /// }))?;
/// # Ok(())
/// # }
    /// ```
    pub fn update<T>(&self, data: T) -> Result<(), FirebaseError> where T: Serialize {
        let data = serde_json::to_string(&data)?;
        self.write_request(Method::Patch, Some(&data))?;
        Ok(())
    }

    /// Delete the value of the reference
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// client.reference("/").delete()?;
/// # Ok(())
/// # }
    /// ```
    pub fn delete(&self) -> Result<(), FirebaseError> {
        self.write_request(Method::Delete, None)?;
        Ok(())
    }

    /// Get the value of the reference as a stream
    /// 
    /// # Example
    /// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use firerust::FirebaseClient;
    /// use serde_json::Value;
    /// 
    /// let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
    /// client.reference("/").on_snapshot(|snapshot: Value| {
    ///     assert_eq!(snapshot["message"].as_str(), Some("Hello, world!"));
    ///     Ok(())
    /// }, |_| {});
/// # Ok(())
/// # }
    /// ```
    pub fn on_snapshot<T, F, E>(&self, callback: F, on_error: E) -> Result<JoinHandle<()>, FirebaseError> where 
        T: Send + 'static,
        F: Send + 'static,
        E: Send + 'static,
        T: Serialize + DeserializeOwned,
        F: Fn(T) -> Result<(), FirebaseError>,
        E: Fn(FirebaseError) -> ()
    {
        let (status, event_stream, mut stream, initial_buffer) = self.client.connector.event_stream(&self.path, None, self.client.api_key.as_deref())?;

        if status.code() != 200 {
            return Err(FirebaseError::new(format!("{} {}", status.code(), status.message())));
        }

        let data = serde_json::from_str::<Value>(event_stream.data())?;

        let snap = match data.get("data") {
            Some(snap) => Arc::new(Mutex::new(snap.clone())),
            None => return Err(FirebaseError::new("Invalid data"))
        };

        match snap.clone().lock() {
            Ok(snap) => {
                let data = serde_json::from_value::<T>(snap.clone())?;
                callback(data)?;
            },
            Err(_) => return Err(FirebaseError::new("Invalid data"))
        };

        Ok(std::thread::spawn(move || {
            let mut buffer = initial_buffer;
            
            loop {
                while let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                    let event_data = buffer[..pos].to_vec();
                    buffer.drain(..pos + 2);

                    let event_stream_str = match String::from_utf8(event_data) {
                        Ok(s) => s,
                        Err(e) => { on_error(FirebaseError::new(e.to_string())); continue; },
                    };
                    
                    let event_stream = match EventStream::try_from(event_stream_str) {
                        Ok(es) => es,
                        Err(e) => { on_error(FirebaseError::new(e.to_string())); continue; },
                    };

                    let data = match serde_json::from_str::<Value>(event_stream.data()) {
                        Ok(data) => data,
                        Err(e) => { on_error(FirebaseError::new(e.to_string())); continue; }
                    };

                    let path = match data["path"].as_str() {
                        Some(path) => match path {
                            "/" => "",
                            _ => path
                        },
                        None => continue
                    };

                    let snapshot = match data.get("data") {
                        Some(snap) => snap.clone(),
                        None => continue
                    };

                    match event_stream.event() {
                        EventType::Put => {
                            let mut snap = match snap.lock() {
                                Ok(snap) => snap,
                                Err(e) => { on_error(FirebaseError::new(e.to_string())); continue; }
                            };

                            let pointer = match snap.pointer_mut(&path) {
                                Some(pointer) => pointer,
                                None => continue
                            };

                            *pointer = snapshot;

                            let data = match serde_json::from_value::<T>(snap.clone()) {
                                Ok(data) => data,
                                Err(e) => { on_error(FirebaseError::new(e.to_string())); continue; },
                            };

                            if let Err(e) = callback(data) { on_error(e); }
                        },
                        EventType::Patch => {
                            let mut snap = match snap.lock() {
                                Ok(snap) => snap,
                                Err(e) => { on_error(FirebaseError::new(e.to_string())); continue; }
                            };

                            let pointer = match snap.pointer_mut(&path) {
                                Some(pointer) => pointer,
                                None => continue
                            };

                            let _ = RealtimeReference::merge_value(pointer, snapshot);

                            let data = match serde_json::from_value::<T>(snap.clone()) {
                                Ok(data) => data,
                                Err(e) => { on_error(FirebaseError::new(e.to_string())); continue; }
                            };

                            if let Err(e) = callback(data) { on_error(e); }
                        },                
                        EventType::Cancel => return,
                        EventType::AuthRevoked => return,
                        EventType::KeepAlive => continue,
                        EventType::Unknown(_) => continue,
                    };
                }

                let mut buf = [0; 1024];
                let len = match stream.read(&mut buf) {
                    Ok(len) => len,
                    Err(_) => break
                };

                if len == 0 {
                    break;
                }

                buffer.extend_from_slice(&buf[..len]);
            }
        }))
    }

    #[doc(hidden)]
    pub fn merge_value(a: &mut Value, b: Value) -> Result<(), FirebaseError> {
        match (a, b) {
            (Value::Object(map_a), Value::Object(map_b)) => {
                for (k, v) in map_b {
                    if v.is_null() {
                        map_a.remove(&k);
                    } else {
                        RealtimeReference::merge_value(map_a.entry(k).or_insert(Value::Null), v)?;
                    }
                }
            }
            (a_ref, new_b) => {
                *a_ref = new_b;
            }
        }

        Ok(())
    }
}


/// Firebase client error
#[derive(Debug)]
pub struct FirebaseError {
    message: String
}

impl FirebaseError {
    fn new(message: impl ToString) -> FirebaseError {
        FirebaseError {
            message: message.to_string()
        }
    }
}

impl Error for FirebaseError {}

impl From<url::ParseError> for FirebaseError { fn from(e: url::ParseError) -> Self { FirebaseError::new(e) } }
impl From<serde_json::Error> for FirebaseError { fn from(e: serde_json::Error) -> Self { FirebaseError::new(e) } }
impl From<connector::ConnectorError> for FirebaseError { fn from(e: connector::ConnectorError) -> Self { FirebaseError::new(e) } }


impl Display for FirebaseError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_value_put() {
        let mut a = json!({"foo": "bar"});
        let b = json!({"baz": "qux"});
        
        // Simulating a Put event (replace completely)
        // Actually, RealtimeReference::merge_value replaces `a` with `b` if `b` is not an Object,
        // OR it recursively updates. 
        // Our merge_value doesn't take EventType. It just merges trees.
        RealtimeReference::<'static>::merge_value(&mut a, b).unwrap();
        assert_eq!(a, json!({"foo": "bar", "baz": "qux"}));
    }

    #[test]
    fn test_merge_value_null_deletes() {
        let mut a = json!({"foo": "bar", "baz": "qux"});
        let b = json!({"baz": serde_json::Value::Null});
        
        RealtimeReference::<'static>::merge_value(&mut a, b).unwrap();
        assert_eq!(a, json!({"foo": "bar"}));
    }
}
