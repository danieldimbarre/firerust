use serde::{ Serialize, Deserialize };
use firerust::FirebaseClient;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let client = FirebaseClient::new(std::env::var("FIREBASE_URL")?)?;
    let reference = client.reference("/data");

    reference.set(Data::new("A simple data"))?;
    println!("{:?}", reference.get::<Data>()?);

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Data {
    message: String,
}

impl Data {
    pub fn new(message: impl ToString) -> Data {
        Data {
            message: message.to_string()
        }
    }
}