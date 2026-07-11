use serde_json::Value;

const REDACTED: &str = "***REDACTED***";

pub fn sanitize_persisted_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| !is_sensitive_key(key))
                .map(|(key, child)| (key.clone(), sanitize_persisted_json(child)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(sanitize_persisted_json).collect()),
        _ => value.clone(),
    }
}

pub fn redact_json_for_display(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, child)| {
                    let value = if is_sensitive_key(key) {
                        Value::String(REDACTED.to_string())
                    } else {
                        redact_json_for_display(child)
                    };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact_json_for_display).collect()),
        _ => value.clone(),
    }
}

pub fn redact_sensitive_text(message: &str) -> String {
    let pattern = regex::RegexBuilder::new(
        r#"(^|[?&{,\s])([\"']?(?:customer[_-]*token|access[_-]*token|token|cookie|authorization|credential|(?:local[_-]*)?storage)[\"']?\s*[:=]\s*[\"']?)(?:bearer\s+)?[^\s\"'<>,;&}\]]+"#,
    )
    .case_insensitive(true)
    .build()
    .expect("sensitive text regex must compile");

    pattern
        .replace_all(message, |captures: &regex::Captures<'_>| {
            format!("{}{}{REDACTED}", &captures[1], &captures[2])
        })
        .into_owned()
}

pub fn safe_error_message(error: impl std::fmt::Display) -> String {
    redact_sensitive_text(&error.to_string())
}

pub fn redact_sensitive_json(value: &Value) -> Value {
    redact_json_for_display(value)
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    normalized.contains("token")
        || normalized.contains("cookie")
        || normalized.contains("authorization")
        || normalized.contains("credential")
        || normalized.contains("storage")
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

    #[test]
    fn persisted_json_recursively_removes_sensitive_key_variants() {
        let raw = json!({
            "TOKEN": "synthetic-token-secret",
            "nested": [{
                "customerToken": "synthetic-customer-secret",
                "customer_token": "synthetic-snake-secret",
                "Access-Token": "synthetic-access-secret",
                "COOKIE": "synthetic-cookie-secret",
                "Authorization": "synthetic-auth-secret",
                "CreDenTial": "synthetic-credential-secret",
                "local_storage": "synthetic-storage-secret",
                "safe": "visible"
            }]
        });

        let sanitized = sanitize_persisted_json(&raw);
        let text = serde_json::to_string(&sanitized).unwrap();

        for secret in [
            "synthetic-token-secret",
            "synthetic-customer-secret",
            "synthetic-snake-secret",
            "synthetic-access-secret",
            "synthetic-cookie-secret",
            "synthetic-auth-secret",
            "synthetic-credential-secret",
            "synthetic-storage-secret",
        ] {
            assert!(!text.contains(secret));
        }
        assert_eq!(sanitized["nested"][0]["safe"], "visible");
        assert!(sanitized["nested"][0].get("customerToken").is_none());
    }

    #[test]
    fn display_json_recursively_replaces_sensitive_values() {
        let raw = json!({
            "safe": "token-style",
            "nested": {"customer_token": "synthetic-display-secret"}
        });

        let redacted = redact_json_for_display(&raw);

        assert_eq!(redacted["safe"], "token-style");
        assert_eq!(redacted["nested"]["customer_token"], "***REDACTED***");
        assert!(!serde_json::to_string(&redacted)
            .unwrap()
            .contains("synthetic-display-secret"));
    }

    #[test]
    fn text_redaction_handles_assignments_queries_json_headers_and_compact_text() {
        let cases = [
            "request failed token=synthetic-token-secret",
            "https://example.invalid/path?safe=1&customer_token=synthetic-query-secret&next=2",
            r#"{"safe":"visible","accessToken":"synthetic-json-secret"}"#,
            "Authorization: Bearer synthetic-bearer-secret",
            "cookie=synthetic-cookie-secret",
            r#"prefix{"credential":"synthetic-compact-secret"}suffix"#,
            "safe=visible&storage=synthetic-storage-secret&token=synthetic-tail-secret",
        ];

        for input in cases {
            let redacted = redact_sensitive_text(input);
            assert!(redacted.contains("***REDACTED***"), "input: {input}");
            assert!(!redacted.contains("synthetic-"), "output: {redacted}");
        }
    }

    #[test]
    fn text_redaction_does_not_redact_ordinary_values_containing_token_style() {
        let input = "asset=token-style map=authorization-center note=cookie-cutter";

        assert_eq!(redact_sensitive_text(input), input);
    }

    #[test]
    fn safe_error_message_redacts_synthetic_secrets() {
        let error = anyhow::anyhow!(
            "upstream failed: token=synthetic-error-secret Authorization: Bearer synthetic-header-secret"
        );

        let message = safe_error_message(error);

        assert!(!message.contains("synthetic-error-secret"));
        assert!(!message.contains("synthetic-header-secret"));
        assert!(message.contains("***REDACTED***"));
    }
}
