use super::*;

#[test]
fn sensitive_fields_and_inline_assignments_are_redacted() {
    assert!(is_sensitive_key("customer_token"));
    assert_eq!(
        sanitize_value("request failed token=abc123 season=23"),
        "request failed token=[REDACTED]"
    );
    assert_eq!(
        sanitize_value(r#"{"authorization":"Bearer-abc","ok":true}"#),
        r#"{"authorization":"[REDACTED]","ok":true}"#
    );
    assert_eq!(
        sanitize_value("authorization: Bearer abc123, status=401"),
        "authorization: [REDACTED], status=401"
    );
}

#[test]
fn writer_rotates_and_keeps_the_configured_backup_count() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("watchback.log");
    let mut writer = LogWriter::open(path.clone(), 12, 2).unwrap();
    writer.write_line(b"first-line\n", true).unwrap();
    writer.write_line(b"second-line\n", true).unwrap();
    writer.write_line(b"third-line\n", true).unwrap();
    assert!(path.exists());
    assert!(backup_path(&path, 1).exists());
    assert!(backup_path(&path, 2).exists());
    assert!(!backup_path(&path, 3).exists());
}
