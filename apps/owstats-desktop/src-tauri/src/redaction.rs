use serde_json::Value;

pub fn redact_sensitive_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (key, child) in map {
                if is_sensitive_key(key) {
                    redacted.insert(key.clone(), Value::String("***REDACTED***".to_string()));
                } else {
                    redacted.insert(key.clone(), redact_sensitive_json(child));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_sensitive_json).collect()),
        _ => value.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("token")
        || normalized.contains("cookie")
        || normalized.contains("authorization")
        || normalized.contains("credential")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_nested_token_fields() {
        let raw = json!({
            "token": "secret",
            "payload": {
                "data": {
                    "customerToken": "customer-secret",
                    "safe": "visible"
                }
            }
        });

        let redacted = redact_sensitive_json(&raw);

        assert_eq!(redacted["token"], "***REDACTED***");
        assert_eq!(
            redacted["payload"]["data"]["customerToken"],
            "***REDACTED***"
        );
        assert_eq!(redacted["payload"]["data"]["safe"], "visible");
        assert!(!serde_json::to_string(&redacted).unwrap().contains("secret"));
    }
}
