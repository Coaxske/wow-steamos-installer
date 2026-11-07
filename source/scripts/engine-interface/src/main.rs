use serde_json::{json, Value};
use std::io::{self, Read, Write};

fn request(command: &str, input: &str) -> Result<(String, Value, Value), String> {
    let envelope: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let object = envelope.as_object().ok_or("Request must be an object")?;
    if object
        .keys()
        .any(|k| !["schema_version", "request_id", "params"].contains(&k.as_str()))
    {
        return Err("Unknown envelope field".into());
    }
    if envelope["schema_version"] != 1 {
        return Err("Expected schema_version 1".into());
    }
    let id = envelope.get("request_id").cloned().unwrap_or(Value::Null);
    if !id.is_null() && !id.is_string() {
        return Err("request_id must be a string".into());
    }
    let mut params = envelope.get("params").cloned().unwrap_or_else(|| json!({}));
    let p = params.as_object_mut().ok_or("params must be an object")?;
    if p.contains_key("action") || p.contains_key("strict_review") {
        return Err("Reserved parameter".into());
    }
    let action = match command {
        "discover" => "scan",
        "status" => "init",
        "repair" => "review_repair",
        "remove" => "review_clean",
        "reinstall" => "review_reinstall",
        "apply" => "apply",
        "recover" => "recover",
        "undo" => "undo",
        "cancel-recovery" => "cancel_cleanup",
        "settings-backups" => "settings_backups",
        "restore-settings" => "restore_settings",
        "adoption-candidates" => "adoption_candidates",
        "plan" => match p
            .remove("operation")
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
        {
            Some("setup") => "review_setup",
            Some("repair") => "review_repair",
            Some("remove") => "review_clean",
            Some("reinstall") => "review_reinstall",
            Some("adopt") => "review_adopt",
            Some("addon") => "review_addon",
            _ => return Err("Unknown or missing plan operation".into()),
        },
        _ => return Err("Unknown command; use interface".into()),
    };
    p.insert("action".into(), json!(action));
    Ok((action.into(), params, id))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    let mut response = json!({"schema_version":1,"interface_version":"1.1.0",
        "engine_version":wow_steamos_core::engine::VERSION,"engine_source_sha256":wow_steamos_core::release::SOURCE,"command":command});
    let code;
    if args.len() == 1 && command == "interface" {
        response["ok"] = json!(true);
        response["data"] = json!({"commands":["discover","status","plan","apply","repair","remove","reinstall","recover","undo","cancel-recovery","settings-backups","restore-settings","adoption-candidates"],
            "exit_codes":{"0":"success","2":"invalid protocol or command","3":"engine safety refusal or operation error","4":"transport error","5":"internal panic"},
            "repair_remove_reinstall":"plan only; apply requires returned token and deletion confirmation where applicable",
            "strict_apply":false});
        code = 0;
    } else {
        let mut bytes = Vec::new();
        let parsed = if args.len() != 1 {
            Err((2, "Specify exactly one command".to_string()))
        } else {
            match io::stdin().take(65537).read_to_end(&mut bytes) {
                Err(e) => Err((4, e.to_string())),
                Ok(_) if bytes.len() > 65536 => Err((2, "Request exceeds 65536 bytes".into())),
                Ok(_) => String::from_utf8(bytes)
                    .map_err(|e| (2, e.to_string()))
                    .and_then(|s| request(command, &s).map_err(|e| (2, e))),
            }
        };
        match parsed {
            Err((exit, message)) => {
                code = exit;
                response["ok"] = json!(false);
                response["error"] =
                    json!({"kind":if exit==4 {"transport"} else {"protocol"},"message":message});
            }
            Ok((action, params, id)) => {
                response["engine_action"] = json!(action);
                response["request_id"] = id;
                match std::panic::catch_unwind(|| wow_steamos_core::engine::dispatch(params)) {
                    Ok(Ok(data)) => {
                        code = 0;
                        response["ok"] = json!(true);
                        response["data"] = data;
                    }
                    Ok(Err(message)) => {
                        code = 3;
                        response["ok"] = json!(false);
                        response["error"] = json!({"kind":"engine","message":message});
                    }
                    Err(_) => {
                        code = 5;
                        response["ok"] = json!(false);
                        response["error"] = json!({"kind":"internal","message":"Engine panic; preserve evidence and stop this operation"});
                    }
                }
            }
        }
    }
    if writeln!(io::stdout().lock(), "{response}").is_err() {
        std::process::exit(4);
    }
    std::process::exit(code);
}
