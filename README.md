# Firerust

A very simple library to implement the Firebase real-time database in your code with the best performance

<p>
    <center>
        <a href="https://crates.io/crates/firerust"><img src="https://img.shields.io/crates/v/firerust.svg" height=22></a>
        <a href="https://github.com/danieldimbarre/Firerust/blob/main/LICENSE"><img src="https://img.shields.io/github/license/danieldimbarre/firerust.svg" height=22></a>
        <a href="https://github.com/danieldimbare/firerust"><img src="https://img.shields.io/github/downloads/danieldimbarre/firerust/total.svg" height=22></a> 
        <a href="https://github.com/danieldimbarre/Firerust/issues"><img src="https://img.shields.io/github/issues/danieldimbarre/firerust.svg" height=22></a>
        <a href="https://github.com/danieldimbarre/Firerust/pulls"><img src="https://img.shields.io/github/issues-pr/danieldimbarre/firerust.svg" height=22></a> 
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