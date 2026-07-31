use anyhow::Context;
use serde_json::{Map, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const LOG_BACKUPS: usize = 3;
const MAX_VALUE_CHARS: usize = 1_000;

static LOGGER: OnceLock<DiagnosticsLogger> = OnceLock::new();
static PANIC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

struct DiagnosticsLogger {
    session_id: String,
    writer: Mutex<LogWriter>,
}

struct LogWriter {
    path: PathBuf,
    file: Option<File>,
    bytes: u64,
    max_bytes: u64,
    backups: usize,
}

impl LogWriter {
    fn open(path: PathBuf, max_bytes: u64, backups: usize) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建日志目录: {}", parent.display()))?;
        }
        let bytes = fs::metadata(&path).map(|value| value.len()).unwrap_or(0);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("无法打开日志文件: {}", path.display()))?;
        Ok(Self {
            path,
            file: Some(file),
            bytes,
            max_bytes,
            backups,
        })
    }

    fn write_line(&mut self, line: &[u8], flush: bool) -> io::Result<()> {
        if self.bytes > 0 && self.bytes.saturating_add(line.len() as u64) > self.max_bytes {
            self.rotate()?;
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("日志文件句柄不可用"))?;
        file.write_all(line)?;
        self.bytes = self.bytes.saturating_add(line.len() as u64);
        if flush {
            file.flush()?;
        }
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        if self.backups > 0 {
            let oldest = backup_path(&self.path, self.backups);
            remove_file_if_exists(&oldest)?;
            for index in (1..self.backups).rev() {
                let source = backup_path(&self.path, index);
                let destination = backup_path(&self.path, index + 1);
                rename_if_exists(&source, &destination)?;
            }
            rename_if_exists(&self.path, &backup_path(&self.path, 1))?;
        } else {
            remove_file_if_exists(&self.path)?;
        }
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );
        self.bytes = 0;
        Ok(())
    }
}

fn backup_path(path: &Path, index: usize) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".{index}"));
    PathBuf::from(value)
}

fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn rename_if_exists(source: &Path, destination: &Path) -> io::Result<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn ensure_log_file(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new().create(true).append(true).open(path)?;
    Ok(())
}

pub(crate) fn initialize(path: PathBuf) -> anyhow::Result<()> {
    if LOGGER.get().is_some() {
        return Ok(());
    }
    let logger = DiagnosticsLogger {
        session_id: uuid::Uuid::new_v4().simple().to_string(),
        writer: Mutex::new(LogWriter::open(path, MAX_LOG_BYTES, LOG_BACKUPS)?),
    };
    let _ = LOGGER.set(logger);
    install_panic_hook();
    Ok(())
}

fn install_panic_hook() {
    PANIC_HOOK_INSTALLED.get_or_init(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic| {
            let message = panic
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.payload().downcast_ref::<String>().map(String::as_str))
                .unwrap_or("non-string panic payload");
            let location = panic
                .location()
                .map(|value| {
                    let source = Path::new(value.file())
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("unknown");
                    format!("{source}:{}", value.line())
                })
                .unwrap_or_else(|| "unknown".to_string());
            record(
                Level::Error,
                "runtime",
                "panic",
                &[("message", message.to_string()), ("location", location)],
            );
            previous(panic);
        }));
    });
}

pub(crate) fn record(level: Level, component: &str, event: &str, fields: &[(&str, String)]) {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    let mut payload = Map::new();
    payload.insert(
        "timestamp".to_string(),
        Value::String(chrono::Utc::now().to_rfc3339()),
    );
    payload.insert(
        "level".to_string(),
        Value::String(level.as_str().to_string()),
    );
    payload.insert(
        "sessionId".to_string(),
        Value::String(logger.session_id.clone()),
    );
    payload.insert(
        "processId".to_string(),
        Value::Number(std::process::id().into()),
    );
    payload.insert(
        "threadId".to_string(),
        Value::String(format!("{:?}", std::thread::current().id())),
    );
    payload.insert(
        "component".to_string(),
        Value::String(sanitize_value(component)),
    );
    payload.insert("event".to_string(), Value::String(sanitize_value(event)));
    for (key, value) in fields {
        let safe_value = if is_sensitive_key(key) {
            "[REDACTED]".to_string()
        } else {
            sanitize_value(value)
        };
        payload.insert(sanitize_key(key), Value::String(safe_value));
    }
    let Ok(mut line) = serde_json::to_vec(&Value::Object(payload)) else {
        return;
    };
    line.push(b'\n');
    if let Ok(mut writer) = logger.writer.lock() {
        let _ = writer.write_line(&line, matches!(level, Level::Warn | Level::Error));
    }
}

pub(crate) fn record_error(
    component: &str,
    event: &str,
    error: &impl std::fmt::Display,
    fields: &[(&str, String)],
) {
    let mut values = Vec::with_capacity(fields.len() + 1);
    values.push(("error", error.to_string()));
    values.extend(fields.iter().cloned());
    record(Level::Error, component, event, &values);
}

fn sanitize_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        .take(64)
        .collect()
}

fn sanitize_value(value: &str) -> String {
    let flattened = value
        .chars()
        .map(|character| {
            if matches!(character, '\r' | '\n' | '\t') {
                ' '
            } else if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .take(MAX_VALUE_CHARS)
        .collect::<String>();
    redact_assignments(flattened)
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "token",
        "authorization",
        "cookie",
        "password",
        "secret",
        "apikey",
        "dts",
        "credential",
    ]
    .iter()
    .any(|candidate| normalized.contains(candidate))
}

fn redact_assignments(mut value: String) -> String {
    for label in [
        "authorization",
        "customer_token",
        "api_key",
        "apikey",
        "password",
        "cookie",
        "secret",
        "token",
        "dts",
    ] {
        let mut search_from = 0;
        loop {
            let lower = value.to_ascii_lowercase();
            let Some(relative) = lower[search_from..].find(label) else {
                break;
            };
            let label_start = search_from + relative;
            let mut cursor = label_start + label.len();
            while value[cursor..].chars().next().is_some_and(|character| {
                character.is_ascii_whitespace() || matches!(character, '"' | '\'' | ':' | '=')
            }) {
                cursor += value[cursor..].chars().next().unwrap().len_utf8();
            }
            let secret_end = value[cursor..]
                .char_indices()
                .find(|(_, character)| {
                    matches!(character, ',' | '&' | ';' | '"' | '\'' | '}' | ']')
                })
                .map_or(value.len(), |(offset, _)| cursor + offset);
            if secret_end > cursor {
                value.replace_range(cursor..secret_end, "[REDACTED]");
                search_from = cursor + "[REDACTED]".len();
            } else {
                search_from = cursor;
            }
            if search_from >= value.len() {
                break;
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
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
}
