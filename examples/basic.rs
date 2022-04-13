use std::error::Error;
use firerust::FirebaseClient;

fn main() -> Result<(), Box<dyn Error>> {
    let client = FirebaseClient::new(std::env::var("FIREBASE_URL")?);
    
    Ok(())
}