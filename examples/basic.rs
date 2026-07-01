use serde::{ Serialize, Deserialize };
use firerust::FirebaseClient;
use serde_json::Value;
use std::error::Error;


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = FirebaseClient::new(std::env::var("FIREBASE_URL")?)?;
    let reference = client.reference("/data");
    
    reference.set(Data::new("A simple data")).await?;
    println!("{:?}", reference.get::<Data>().await?);

    reference.update(serde_json::json!({
        "message": "Updating data"
    })).await?;
    println!("{:?}", reference.get::<Value>().await?);

    reference.delete().await?;
    println!("{:?}", reference.get::<Value>().await?);

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