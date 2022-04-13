#[derive(Debug, Clone)]
pub struct FirebaseClient {
    url: String,
    api_key: Option<String>,
}

impl FirebaseClient {
    pub fn new(url: impl ToString) -> FirebaseClient {
        FirebaseClient {
            url: url.to_string(),
            api_key: None,
        }
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
}