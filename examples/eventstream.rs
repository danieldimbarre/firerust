use serde::{ Serialize, Deserialize };
use firerust::FirebaseClient;
use serde_json::Value;
use std::error::Error;


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = FirebaseClient::new(std::env::var("FIREBASE_URL")?)?;
    let reference = client.reference("/data");
    
    reference.on_snapshot(| data: Value | {
        println!("Value: {:?}", data);
        Ok(())
    }, |err| eprintln!("Error: {}", err)).await?;

    reference.on_snapshot(| data: Data | {
        println!("Data: {:?}", data);
        Ok(())
    }, |err| eprintln!("Error: {}", err)).await?.await.unwrap();

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Data {
    message: String,
}