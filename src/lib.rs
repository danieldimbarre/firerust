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
}