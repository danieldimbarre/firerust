use firerust::FirebaseClient;
use std::error::Error;
use serde_json::Value;

fn main() -> Result<(), Box<dyn Error>> {
    let mut client = FirebaseClient::new(std::env::var("FIREBASE_URL")?)?;
    client.auth(std::env::var("FIREBASE_API_KEY")?);

    let reference = client.reference("/");

    println!("{:?}", reference.get::<Value>()?);

    Ok(())
}