//! Rolling application log.
//!
//! Native targets append to `<app_data_dir>/logs/pinpoint.log`; desktop also
//! keeps the stderr layer. Android gets the file layer so a crash report can
//! include the last useful lines. wasm is a no-op.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;

const MAX_LOG_BYTES: u64 = 1_048_576;
static INIT: Once = Once::new();
static LOG_FILE: OnceLock<Arc<Mutex<File>>> = OnceLock::new();

#[derive(Clone)]
struct SharedFile(Arc<Mutex<File>>);

impl Write for SharedFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

#[derive(Clone)]
struct FileMakeWriter(Arc<Mutex<File>>);

impl<'a> MakeWriter<'a> for FileMakeWriter {
    type Writer = SharedFile;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFile(self.0.clone())
    }
}

pub fn log_dir() -> Option<PathBuf> {
    crate::platform::app_data_dir().map(|base| base.join("logs"))
}

pub fn log_path() -> Option<PathBuf> {
    log_dir().map(|dir| dir.join("pinpoint.log"))
}

/// Install the process-wide subscriber once. A missing app-data directory or
/// unopenable log file leaves the app running without file logging.
pub fn init() {
    INIT.call_once(|| {
        let Some(path) = log_path() else { return };
        let Some(dir) = path.parent() else { return };
        if fs::create_dir_all(dir).is_err() {
            return;
        }
        rotate_if_needed(&path);
        let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
            return;
        };
        let file = Arc::new(Mutex::new(file));
        let _ = LOG_FILE.set(file.clone());
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(FileMakeWriter(file));
        let registry = tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::INFO)
            .with(file_layer);
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        let registry = registry.with(tracing_subscriber::fmt::layer());
        let _ = registry.try_init();
    });
}

/// Last `max_lines` lines of the current log, or an empty string when no log
/// exists. The 1 MiB rotation bounds the read.
pub fn tail(max_lines: usize) -> String {
    let Some(path) = log_path() else {
        return String::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    tail_lines(&text, max_lines)
}

pub fn clear() {
    if let Some(file) = LOG_FILE.get() {
        if let Ok(mut file) = file.lock() {
            let _ = truncate_open(&mut file);
        }
    } else if let Some(path) = log_path() {
        let _ = fs::remove_file(&path);
    }
    if let Some(path) = log_path() {
        let _ = fs::remove_file(path.with_extension("log.1"));
    }
}

fn truncate_open(file: &mut File) -> io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(())
}

fn rotate_if_needed(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() <= MAX_LOG_BYTES {
        return;
    }
    let rotated = path.with_extension("log.1");
    let _ = fs::remove_file(&rotated);
    let _ = fs::rename(path, &rotated);
    let _ = fs::write(path, b"");
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::tail_lines;

    #[test]
    fn tail_returns_only_the_last_lines() {
        let text = "one\ntwo\nthree\nfour\nfive\n";
        assert_eq!(tail_lines(text, 2), "four\nfive");
    }

    #[test]
    fn tail_handles_crlf_and_empty_input() {
        assert_eq!(tail_lines("one\r\ntwo\r\nthree\r\n", 2), "two\nthree");
        assert_eq!(tail_lines("", 10), "");
    }

    #[test]
    fn tail_zero_lines_is_empty() {
        assert_eq!(tail_lines("one\ntwo\n", 0), "");
    }

    #[test]
    fn rotation_keeps_one_previous_file() {
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-log-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pinpoint.log");
        std::fs::write(&path, vec![b'x'; 1_100_000]).unwrap();
        super::rotate_if_needed(&path);
        assert!(dir.join("pinpoint.log.1").exists());
        assert!(!path.exists() || std::fs::metadata(&path).unwrap().len() == 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_truncates_the_open_writer() {
        use std::io::Write;

        let dir = std::env::temp_dir().join(format!(
            "pinpoint-log-clear-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pinpoint.log");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .unwrap();
        file.write_all(b"before\n").unwrap();
        super::truncate_open(&mut file).unwrap();
        file.write_all(b"after\n").unwrap();
        drop(file);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after\n");
        std::fs::remove_dir_all(dir).ok();
    }
}
