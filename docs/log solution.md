对于支持 no_std 环境的 Rust 库的 debug 日志方案，最佳实践如下：

## 1. **核心方案：log crate + 自定义后端**

### 基础配置
```toml
# Cargo.toml
[dependencies]
log = { version = "0.4", features = ["max_level_debug", "release_max_level_warn"] }

[features]
default = ["std"]
std = ["dep:std_log_backend"]  # 有std时的后端
log-debug = []  # 启用debug日志的feature
```

## 2. **库内部实现**

### 条件编译的日志模块
```rust
// src/log.rs 或 src/debug.rs
#[cfg(feature = "log-debug")]
macro_rules! debug {
    ($($arg:tt)*) => {
        log::debug!($($arg)*)
    };
}

#[cfg(not(feature = "log-debug"))]
macro_rules! debug {
    ($($arg:tt)*) => {{}};  // 完全移除debug日志代码
}
```

### 库代码中使用
```rust
use crate::log::debug;

pub fn some_function() {
    debug!("进入函数，参数: {:?}", some_value);
    // ... 函数逻辑
    debug!("函数完成，结果: {}", result);
}
```

## 3. **no_std 环境适配**

### 抽象日志 trait
```rust
// src/log/mod.rs
pub trait Logger: Sync {
    fn log(&self, level: log::Level, args: core::fmt::Arguments);
    fn flush(&self);
}

// 为 log::Log 实现适配器
pub struct LogAdapter<T: Logger>(pub T);

impl<T: Logger> log::Log for LogAdapter<T> {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        self.0.log(record.level(), *record.args());
    }

    fn flush(&self) {
        self.0.flush();
    }
}
```

### 多个后端实现示例
```rust
// src/log/backends.rs

// 1. ITM (Instrumentation Trace Macrocell) - 用于 ARM Cortex-M
#[cfg(feature = "itm")]
pub struct ItmLogger {
    port: u8,
}

// 2. Semihosting
#[cfg(feature = "semihosting")]
pub struct SemihostingLogger;

// 3. 串口日志
pub struct SerialLogger<T: embedded_hal::serial::Write<u8>> {
    serial: T,
    buffer: heapless::Vec<u8, 256>,
}

// 4. RTT (Real-Time Transfer)
#[cfg(feature = "rtt")]
pub struct RttLogger {
    up_channel: rtt_target::UpChannel,
}
```

## 4. **完整的配置方案**

### 条件编译配置
```rust
// src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "log")]
mod log;

#[cfg(all(feature = "log", feature = "log-debug"))]
#[macro_export]
macro_rules! dbg {
    ($($arg:tt)*) => {
        crate::log::debug!($($arg)*)
    };
}

#[cfg(not(feature = "log-debug"))]
#[macro_export]
macro_rules! dbg {
    ($($arg:tt)*) => {
        // 编译时完全移除
        let _ = (|| { $(let _ = &$arg;)* })();
    };
}
```

### 日志级别控制
```rust
// src/config.rs
pub struct LogConfig {
    pub level: LogLevel,
    pub module_filter: Option<&'static str>,
}

#[derive(PartialEq, PartialOrd)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

// 编译时级别选择
#[cfg(debug_assertions)]
pub const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Debug;

#[cfg(not(debug_assertions))]
pub const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Warn;
```

## 5. **使用示例**

### 库用户代码
```rust
// 用户选择后端
#[cfg(not(feature = "std"))]
fn setup_logging() {
    let serial = // 初始化串口
    let logger = SerialLogger::new(serial);
    log::set_logger(&LogAdapter(logger)).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
}

// 使用宏
use your_library::dbg;

fn main() {
    dbg!("初始化开始");
    // ...
}
```

## 6. **最佳实践建议**

1. **功能开关**：使用 Cargo features 控制日志级别
   ```toml
   [features]
   debug-logging = []  # 启用详细日志
   release-logging = []  # 只启用错误和警告
   ```

2. **零成本抽象**：在 release 构建中完全移除 debug 日志

3. **格式化优化**：
   ```rust
   // 避免在 release 中计算格式化参数
   #[cfg(feature = "log-debug")]
   debug!("状态: {}", expensive_format(args));
   
   // 使用延迟计算
   debug!(|| format!("状态: {}", expensive()));
   ```

4. **模块化设计**：
   ```
   your_library/
   ├── src/
   │   ├── log/
   │   │   ├── mod.rs      # 日志接口
   │   │   ├── macros.rs   # 日志宏
   │   │   └── backends/   # 各种后端实现
   │   └── lib.rs
   └── Cargo.toml
   ```

5. **性能考虑**：
   - 使用 `heapless` 或 `arrayvec` 避免动态分配
   - 提供同步和异步日志接口
   - 支持日志缓冲以减少上下文切换

这个方案提供了灵活性，用户可以根据需要选择不同的后端，同时在编译时可以完全移除 debug 日志以减少代码大小和运行时开销。