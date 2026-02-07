use core::fmt::Write;

#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(feature = "std")]
pub use tracing::{debug, error, info, trace, warn};

#[cfg(all(not(feature = "std"), feature = "log"))]
pub use tracing_core::{debug, error, info, trace, warn};

#[cfg(not(any(feature = "std", feature = "log")))]
macro_rules! debug {
    ($($arg:tt)*) => {
        let _ = (|| { $(let _ = &$arg;)* })();
    };
}

#[cfg(not(any(feature = "std", feature = "log")))]
macro_rules! info {
    ($($arg:tt)*) => {
        let _ = (|| { $(let _ = &$arg;)* })();
    };
}

#[cfg(not(any(feature = "std", feature = "log")))]
macro_rules! warn {
    ($($arg:tt)*) => {
        let _ = (|| { $(let _ = &$arg;)* })();
    };
}

#[cfg(not(any(feature = "std", feature = "log")))]
macro_rules! error {
    ($($arg:tt)*) => {
        let _ = (|| { $(let _ = &$arg;)* })();
    };
}

#[cfg(not(any(feature = "std", feature = "log")))]
macro_rules! trace {
    ($($arg:tt)*) => {
        let _ = (|| { $(let _ = &$arg;)* })();
    };
}

#[cfg(all(not(feature = "std"), feature = "log"))]
pub struct NoStdLogger {
    buffer: heapless::Vec<u8, 256>,
}

#[cfg(all(not(feature = "std"), feature = "log"))]
impl NoStdLogger {
    pub const fn new() -> Self {
        Self {
            buffer: heapless::Vec::new(),
        }
    }

    fn flush_buffer(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        unsafe {
            let data = self.buffer.as_ptr();
            let len = self.buffer.len();

            #[cfg(feature = "posix")]
            {
                use std::io::Write;
                let _ = std::io::stdout().write_all(core::slice::from_raw_parts(data, len));
            }

            #[cfg(feature = "baremetal")]
            {
                let _ = crate::platform::file_write(
                    1 as crate::platform::FileHandle,
                    data,
                    len,
                );
            }
        }

        self.buffer.clear();
    }
}

#[cfg(all(not(feature = "std"), feature = "log"))]
impl Write for NoStdLogger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            if self.buffer.push(byte).is_err() {
                self.flush_buffer();
                self.buffer.push(byte).map_err(|_| core::fmt::Error)?;
            }
        }
        Ok(())
    }
}

#[cfg(all(not(feature = "std"), feature = "log"))]
impl tracing_core::Subscriber for NoStdLogger {
    fn enabled(&self, metadata: &tracing_core::Metadata) -> bool {
        #[cfg(debug_assertions)]
        return metadata.level() <= &tracing_core::Level::DEBUG;

        #[cfg(not(debug_assertions))]
        return metadata.level() <= &tracing_core::Level::WARN;
    }

    fn new_span(&self, _span: &tracing_core::span::Attributes) -> tracing_core::span::Id {
        tracing_core::span::Id::from_u64(0)
    }

    fn record(&self, _span: &tracing_core::span::Id, _values: &tracing_core::span::Record) {}

    fn record_follows_from(&self, _span: &tracing_core::span::Id, _follows: &tracing_core::span::Id) {}

    fn event(&self, event: &tracing_core::Event) {
        if !self.enabled(event.metadata()) {
            return;
        }

        let mut visitor = EventVisitor {
            logger: self,
            timestamp: crate::platform::get_timestamp(),
        };

        event.record(&mut visitor);
        visitor.logger.flush_buffer();
    }

    fn enter(&self, _span: &tracing_core::span::Id) {}

    fn exit(&self, _span: &tracing_core::span::Id) {}
}

#[cfg(all(not(feature = "std"), feature = "log"))]
struct EventVisitor<'a> {
    logger: &'a mut NoStdLogger,
    timestamp: u64,
}

#[cfg(all(not(feature = "std"), feature = "log"))]
impl<'a> tracing_core::field::Visit for EventVisitor<'a> {
    fn record_debug(&mut self, field: &tracing_core::field::Field, value: &dyn core::fmt::Debug) {
        let _ = write!(self.logger.buffer, "[{}ms] ", self.timestamp);
        let _ = write!(self.logger.buffer, "{}: {:?}\n", field.name(), value);
    }

    fn record_f64(&mut self, field: &tracing_core::field::Field, value: f64) {
        let _ = write!(self.logger.buffer, "[{}ms] ", self.timestamp);
        let _ = write!(self.logger.buffer, "{}: {}\n", field.name(), value);
    }

    fn record_i64(&mut self, field: &tracing_core::field::Field, value: i64) {
        let _ = write!(self.logger.buffer, "[{}ms] ", self.timestamp);
        let _ = write!(self.logger.buffer, "{}: {}\n", field.name(), value);
    }

    fn record_u64(&mut self, field: &tracing_core::field::Field, value: u64) {
        let _ = write!(self.logger.buffer, "[{}ms] ", self.timestamp);
        let _ = write!(self.logger.buffer, "{}: {}\n", field.name(), value);
    }

    fn record_bool(&mut self, field: &tracing_core::field::Field, value: bool) {
        let _ = write!(self.logger.buffer, "[{}ms] ", self.timestamp);
        let _ = write!(self.logger.buffer, "{}: {}\n", field.name(), value);
    }

    fn record_str(&mut self, field: &tracing_core::field::Field, value: &str) {
        let _ = write!(self.logger.buffer, "[{}ms] ", self.timestamp);
        let _ = write!(self.logger.buffer, "{}: {}\n", field.name(), value);
    }

    fn record_error(&mut self, field: &tracing_core::field::Field, value: &(dyn core::error::Error + 'static)) {
        let _ = write!(self.logger.buffer, "[{}ms] ", self.timestamp);
        let _ = write!(self.logger.buffer, "{}: {}\n", field.name(), value);
    }
}

#[cfg(all(not(feature = "std"), feature = "log"))]
static GLOBAL_LOGGER: core::sync::OnceLock<NoStdLogger> = core::sync::OnceLock::new();

#[cfg(feature = "std")]
pub fn init_logger() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .init();
}

#[cfg(feature = "std")]
pub fn init_logger_with_file(log_path: &str, debug_mode: bool) -> Result<(), std::io::Error> {
    use tracing::Level;

    let log_level = if debug_mode { Level::DEBUG } else { Level::INFO };

    let env_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(format!("remdb={}", log_level).parse().unwrap())
        .add_directive(format!("remdb_server={}", log_level).parse().unwrap());

    tracing_subscriber::fmt()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_level(true)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(env_filter)
        .init();

    Ok(())
}

#[cfg(all(not(feature = "std"), feature = "log"))]
pub fn init_logger() {
    let logger = NoStdLogger::new();
    GLOBAL_LOGGER.set(logger).ok();
    tracing_core::dispatcher::set_global_default(tracing_core::dispatcher::Dispatch::new(
        GLOBAL_LOGGER.get().unwrap(),
    ))
    .ok();
}

#[cfg(not(any(feature = "std", feature = "log")))]
pub fn init_logger() {}
