# Firerust

A very simple library to implement the Firebase real-time database in your code with the best performance

<p>
    <center>
        <a href="https://crates.io/crates/firerust"><img src="https://img.shields.io/crates/v/firerust.svg?style=for-the-badge&logo=rust" height=22></a>
        <a href="https://docs.rs/firerust/latest/firerust/"><img src="https://img.shields.io/badge/docs.rs-Firerust-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white&logo=docs.rs" height=22></a>
        <a href="https://github.com/danieldimbarre/Firerust/blob/main/LICENSE"><img src="https://img.shields.io/github/license/danieldimbarre/firerust.svg?style=for-the-badge" height=22></a>
        <a href="https://github.com/danieldimbare/firerust"><img src="https://img.shields.io/crates/dv/firerust?style=for-the-badge" height=22></a> 
        <a href="https://signal.group/#CjQKIEut8ZhBy03B3v3eN2EvQxuDjGE21rSAOHvahJJ9FFgTEhAvnRZs_vqd1PrxL16iy7m9"><img src="https://img.shields.io/badge/Signal-Firerust-00B2FF?style=for-the-badge&logo=signal&logoColor=white&labelColor=555555" height=22></a>
    </center>
</p>

# Instalation
Add this to your `Cargo.toml`:
```toml
[dependencies]
firerust = { version = "1" }
```

# Usage
Import firerust
```rust 
use firerust::FirebaseClient;
```

Initialize a Firebase client without auth
```rust
FirebaseClient::new("https:///<DATABASE_NAME>.firebaseio.com/")?;
```

Initialize a Firebase client with auth
```rust
let mut client = FirebaseClient::new("https:///<DATABASE_NAME>.firebaseio.com/")?;
client.auth("<ID_TOKEN>");
```

# Examples

A basic example of data fetch:
```rust
use firerust::FirebaseClient;
use serde_json::Value;


let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
let reference = client.reference("/");

println!("{:?}", reference.get::<Value>());
```

A basic example of data set:
```rust
use firerust::FirebaseClient;


let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
let reference = client.reference("/");

reference.set(serde_json::json!({
    "message": "Setting data"
}))?;
```

A basic example of data update:
```rust
use firerust::FirebaseClient;


let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
let reference = client.reference("/");

reference.update(serde_json::json!({
    "message": "Updating data"
}))?;
```

A basic example of data deletion:
```rust
use firerust::FirebaseClient;


let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
let reference = client.reference("/");

reference.delete()?;
```

A snapshot event example:
```rust
use firerust::FirebaseClient;
use serde_json::Value;


let client = FirebaseClient::new("https://docs-examples.firebaseio.com/")?;
let reference = client.reference("/");

reference.on_snapshot(| data: Value | {
    println!("{:?}", data);

    Ok(())
})?;
```