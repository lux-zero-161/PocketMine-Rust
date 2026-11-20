use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::text::to_ansi;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Emergency,
}

impl LogLevel {
    pub fn name(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Notice => "NOTICE",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
            LogLevel::Critical => "CRITICAL",
            LogLevel::Emergency => "EMERGENCY",
        }
    }

    pub fn color_code(&self) -> &'static str {
        match self {
            LogLevel::Debug => "\x1b[38;5;244m",
            LogLevel::Info => "\x1b[36m",
            LogLevel::Notice => "\x1b[32m",
            LogLevel::Warning => "\x1b[33m",
            LogLevel::Error => "\x1b[31m",
            LogLevel::Critical => "\x1b[41;37m",
            LogLevel::Emergency => "\x1b[45;37m",
        }
    }

    pub fn from_int(level: i32) -> Self {
        match level {
            0 => LogLevel::Debug,
            1 => LogLevel::Info,
            2 => LogLevel::Notice,
            3 => LogLevel::Warning,
            4 => LogLevel::Error,
            _ => LogLevel::Critical,
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Notice => 2,
            LogLevel::Warning => 3,
            LogLevel::Error => 4,
            LogLevel::Critical => 5,
            LogLevel::Emergency => 6,
        }
    }
}

struct LoggerInner {
    thread_name: String,
    min_level: LogLevel,
    file: Option<File>,
    file_path: Option<PathBuf>,
    timestamps: bool,
    colored: bool,
}

/// Thread-safe logger with optional file mirroring.
///
/// Mirrors the behaviour of `pocketmine\utils\MainLogger`: everything is printed
/// to stdout and can additionally be mirrored into `logs/server.log`.
pub struct Logger {
    inner: Arc<Mutex<LoggerInner>>,
}

impl Logger {
    pub fn new(thread_name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(LoggerInner {
                thread_name: thread_name.into(),
                min_level: LogLevel::Debug,
                file: None,
                file_path: None,
                timestamps: true,
                colored: true,
            })),
        }
    }

    /// Creates a logger that prefixes messages with `name`, like PMMP's
    /// `Server::getChildLogger()`.
    pub fn child(&self, name: impl Into<String>) -> Logger {
        let name = name.into();
        let parent = self.clone();
        let logger = Logger::new(name);
        if let Ok(parent_inner) = parent.inner.lock() {
            if let Ok(mut inner) = logger.inner.lock() {
                inner.min_level = parent_inner.min_level;
                inner.timestamps = parent_inner.timestamps;
                inner.colored = parent_inner.colored;
            }
        }
        logger
    }

    pub fn set_min_level(&self, level: LogLevel) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.min_level = level;
        }
    }

    pub fn set_colored(&self, colored: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.colored = colored;
        }
    }

    /// Mirrors all log output into `path` (append mode).
    pub fn attach_file(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let mut inner = self.inner.lock().unwrap();
        inner.file = Some(file);
        inner.file_path = Some(path.to_path_buf());
        Ok(())
    }

    fn timestamp() -> (String, String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let total = now.as_secs();
        let (h, m, s) = ((total / 3600) % 24, (total / 60) % 60, total % 60);
        let date = {
            // Days since epoch -> Y-M-D (civil from days algorithm).
            let days = (total / 86_400) as i64;
            let z = days + 719_468;
            let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let mo = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = if mo <= 2 { y + 1 } else { y };
            format!("{:04}-{:02}-{:02}", y, mo, d)
        };
        (
            date,
            format!("{:02}:{:02}:{:02}.{:03}", h, m, s, now.subsec_millis()),
        )
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if level < inner.min_level {
            return;
        }
        let (date, time) = Self::timestamp();
        let plain = crate::text::strip_colors(message);

        if inner.timestamps {
            println!(
                "\x1b[38;5;240m[{} {}]\x1b[0m [{}/{}{}{}]: {}",
                date,
                time,
                inner.thread_name,
                level.color_code(),
                level.name(),
                "\x1b[0m",
                if inner.colored {
                    to_ansi(message)
                } else {
                    message.to_string()
                }
            );
        } else {
            println!(
                "[{}/{}{}{}]: {}",
                inner.thread_name,
                level.color_code(),
                level.name(),
                "\x1b[0m",
                if inner.colored {
                    to_ansi(message)
                } else {
                    message.to_string()
                }
            );
        }

        let thread_name = inner.thread_name.clone();
        if let Some(file) = inner.file.as_mut() {
            let _ = writeln!(
                file,
                "[{} {}] [{}/{}]: {}",
                date,
                time,
                thread_name,
                level.name(),
                plain
            );
            let _ = file.flush();
        }
    }

    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }
    pub fn notice(&self, message: &str) {
        self.log(LogLevel::Notice, message);
    }
    pub fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message);
    }
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
    pub fn critical(&self, message: &str) {
        self.log(LogLevel::Critical, message);
    }
}

impl Clone for Logger {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new("Server thread")
    }
}
