use std::fmt;
use std::time::SystemTime;

// 日志级别枚举
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warning => write!(f, "WARNING"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

// 日志配置结构体
pub struct LogConfig {
    pub level: LogLevel,
    pub show_timestamp: bool,
    pub show_level: bool,
    pub show_module: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            show_timestamp: true,
            show_level: true,
            show_module: true,
        }
    }
}

// 日志记录器结构体
pub struct Logger {
    config: LogConfig,
    module_name: String,
}

impl Logger {
    // 创建新的日志记录器
    pub fn new(module_name: &str, config: Option<LogConfig>) -> Self {
        let config = config.unwrap_or_default();
        Self {
            config,
            module_name: module_name.to_string(),
        }
    }

    // 记录调试日志
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    // 记录信息日志
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    // 记录警告日志
    pub fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message);
    }

    // 记录错误日志
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    // 内部日志记录方法
    fn log(&self, level: LogLevel, message: &str) {
        // 检查日志级别是否大于等于配置的级别
        if self.should_log(level) {
            let mut log_line = String::new();

            // 添加时间戳
            if self.config.show_timestamp {
                let now = SystemTime::now();
                let timestamp = now.duration_since(SystemTime::UNIX_EPOCH)
                    .expect("Failed to get system time")
                    .as_millis();
                log_line.push_str(&format!("[{}] ", timestamp));
            }

            // 添加日志级别
            if self.config.show_level {
                log_line.push_str(&format!("[{}] ", level));
            }

            // 添加模块名称
            if self.config.show_module {
                log_line.push_str(&format!("[{}] ", self.module_name));
            }

            // 添加日志消息
            log_line.push_str(message);

            // 输出日志
            match level {
                LogLevel::Debug | LogLevel::Info => println!("{}", log_line),
                LogLevel::Warning | LogLevel::Error => eprintln!("{}", log_line),
            }
        }
    }

    // 检查是否应该记录该级别的日志
    fn should_log(&self, level: LogLevel) -> bool {
        match (self.config.level, level) {
            (LogLevel::Debug, _) => true,
            (LogLevel::Info, LogLevel::Info | LogLevel::Warning | LogLevel::Error) => true,
            (LogLevel::Warning, LogLevel::Warning | LogLevel::Error) => true,
            (LogLevel::Error, LogLevel::Error) => true,
            _ => false,
        }
    }
}
