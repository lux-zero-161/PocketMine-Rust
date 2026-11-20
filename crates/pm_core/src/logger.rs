use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            LogLevel::Debug => "\x1b[38;5;244m", // Gray
            LogLevel::Info => "\x1b[36m",         // Cyan
            LogLevel::Notice => "\x1b[32m",       // Green
            LogLevel::Warning => "\x1b[33m",      // Yellow
            LogLevel::Error => "\x1b[31m",        // Red
            LogLevel::Critical => "\x1b[41;37m",  // Red background, white text
            LogLevel::Emergency => "\x1b[45;37m", // Magenta background
        }
    }
}

pub struct Logger {
    thread_name: String,
    lock: Arc<Mutex<()>>,
}

impl Logger {
    pub fn new(thread_name: impl Into<String>) -> Self {
        Self {
            thread_name: thread_name.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        let _guard = self.lock.lock().unwrap();

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let total_secs = now.as_secs();
        let millis = now.subsec_millis();
        let hours = (total_secs / 3600) % 24;
        let mins = (total_secs / 60) % 60;
        let secs = total_secs % 60;

        let time_str = format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis);
        let color = level.color_code();
        let reset = "\x1b[0m";

        println!(
            "\x1b[38;5;240m[{}]\x1b[0m [{}/{}{}{}]: {}",
            time_str, self.thread_name, color, level.name(), reset, message
        );
    }

    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    pub fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message);
    }

    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    pub fn notice(&self, message: &str) {
        self.log(LogLevel::Notice, message);
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new("Server thread")
    }
}
