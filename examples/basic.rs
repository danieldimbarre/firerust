use firerust::FirebaseClient;
use std::error::Error;
use serde_json::Value;

fn main() -> Result<(), Box<dyn Error>> {
    let client = FirebaseClient::new(std::env::var("FIREBASE_URL")?)?;
    let reference = client.reference("/");

    println!("{:?}", reference.get::<Value>()?);

    Ok(())
}