use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<File>> = OnceLock::new();
const MAX_SIZE: u64 = 256 * 1024;

fn open(path: &Path) -> io::Result<File> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Log has no parent"))?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    if file.metadata()?.len() > MAX_SIZE {
        file.set_len(0)?;
    }
    Ok(file)
}

pub fn init() -> io::Result<()> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is unset"))?;
    let path = std::path::PathBuf::from(home).join(".local/state/omarchy/tidal/daemon.log");
    LOG.set(Mutex::new(open(&path)?))
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "Log already initialized"))
}

pub fn safe_text(text: &str) -> String {
    let mut words = Vec::new();
    for word in text.split_whitespace().take(80) {
        let lower = word.to_ascii_lowercase();
        if lower.contains("authorization")
            || lower.contains("bearer")
            || lower.contains("_token")
            || lower.contains("token=")
            || lower == "token"
        {
            words.push("[redacted]");
            break;
        }
        words.push(if lower.contains("://") {
            "[redacted-url]"
        } else {
            word
        });
    }
    words.join(" ")
}

pub fn http_failure(path: &str, error: &ureq::Error) {
    match error {
        ureq::Error::Status(code, _) => write(&format!("API {path} failed: HTTP {code}")),
        ureq::Error::Transport(error) => {
            write(&format!("API {path} transport failure: {:?}", error.kind()))
        }
    }
}

pub fn write(message: &str) {
    let line = format!(
        "[{}] {}",
        crate::auth::current_unix_timestamp(),
        safe_text(message)
    );
    eprintln!("{line}");
    if let Some(log) = LOG.get() {
        match log.lock() {
            Ok(mut file) => {
                if let Err(error) = writeln!(file, "{line}") {
                    eprintln!("Failed to write daemon log: {error}");
                }
            }
            Err(error) => eprintln!("Failed to lock daemon log: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_log_append_and_startup_truncation() {
        let dir = std::env::temp_dir().join(format!("tidal-log-test-{}", std::process::id()));
        let path = dir.join("daemon.log");
        let mut file = open(&path).unwrap();
        file.write_all(b"first\n").unwrap();
        drop(file);
        let mut file = open(&path).unwrap();
        file.write_all(b"second\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        file.set_len(MAX_SIZE).unwrap();
        drop(file);
        let file = open(&path).unwrap();
        assert_eq!(file.metadata().unwrap().len(), MAX_SIZE);
        file.set_len(MAX_SIZE + 1).unwrap();
        drop(file);
        let file = open(&path).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 0);
        drop(file);
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn test_log_redaction() {
        assert_eq!(
            safe_text(
                "failed https://cdn.example/stream?secret=token\nAuthorization: Bearer secret"
            ),
            "failed [redacted-url] [redacted]"
        );
        assert_eq!(
            safe_text("failed refresh_token=secret"),
            "failed [redacted]"
        );
    }
}
