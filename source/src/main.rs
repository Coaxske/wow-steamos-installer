use std::io::{self, Read};
fn main() {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("read request");
    let result = serde_json::from_str(&input)
        .map_err(|e| e.to_string())
        .and_then(wow_steamos_core::engine::dispatch);
    let response = match result {
        Ok(v) => serde_json::json!({"ok":true,"data":v}),
        Err(e) => serde_json::json!({"ok":false,"error":e}),
    };
    println!("{}", response);
    if response["ok"] == false {
        std::process::exit(1);
    }
}
