# remdb 日志系统使用指南

## 概述

remdb 使用 `tracing` 框架统一管理日志，同时完全支持 `no_std` 环境。通过条件编译，可以在不同环境下自动选择合适的日志实现。

## 架构设计

```
┌─────────────────────────────────────┐
│     remdb 代码 (使用 tracing! 宏)    │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│      日志抽象层 (src/log.rs)         │
│  - 条件编译的宏定义                   │
│  - 日志级别控制                       │
└──────────────┬──────────────────────┘
               │
       ┌───────┴───────┐
       │               │
┌──────▼──────┐  ┌─────▼──────┐
│ std 环境    │  │ no_std 环境 │
│ tracing-    │  │ 自定义      │
│ subscriber  │  │ subscriber  │
└─────────────┘  └─────────────┘
```

## 特性说明

### Cargo Features

| 特性名 | 依赖 | 描述 |
|-------|------|------|
| `log` | - | 启用日志功能（默认启用） |
| `std` | `log` | 启用标准库支持，使用完整的 tracing 生态 |
| `baremetal` | `log`, `tracing-core`, `heapless` | 启用裸机环境支持 |

## 使用方法

### 1. 在 Cargo.toml 中添加依赖

```toml
[dependencies]
remdb = { path = "./remdb", default-features = false }

# 标准环境
remdb = { path = "./remdb", features = ["std", "log"] }

# 裸机环境
remdb = { path = "./remdb", features = ["baremetal", "log"] }

# 禁用日志（最小二进制大小）
remdb = { path = "./remdb", default-features = false }
```

### 2. 初始化日志

```rust
use remdb::log::init_logger;

fn main() {
    // 初始化日志系统
    init_logger();

    // ... 你的代码
}
```

### 3. 使用日志宏

```rust
use remdb::log::{debug, error, info, trace, warn};

pub fn some_function() {
    info!("进入函数");
    debug!("调试信息: {:?}", some_value);
    warn!("警告信息");
    error!("错误发生: {}", error_msg);
    trace!("详细追踪信息");
}
```

## 日志级别

| 级别 | 用途 | std 环境 | no_std 环境 |
|------|------|----------|-------------|
| `trace!` | 最详细的追踪信息 | ✅ | ❌ (编译时移除) |
| `debug!` | 调试信息 | ✅ | ✅ (仅 debug 模式) |
| `info!` | 一般信息 | ✅ | ✅ |
| `warn!` | 警告信息 | ✅ | ✅ |
| `error!` | 错误信息 | ✅ | ✅ |

## 环境适配

### std 环境

```rust
#[cfg(feature = "std")]
fn main() {
    init_logger();
    
    // 使用 tracing-subscriber 提供的完整功能
    // - 彩色输出
    // - 时间戳
    // - 模块路径
    // - 环境变量过滤 (RUST_LOG)
    
    info!("标准环境日志");
}
```

### no_std 环境

```rust
#![no_std]

use remdb::log::{info, init_logger};

#[entry]
fn main() -> ! {
    init_logger();
    
    // 使用自定义的 NoStdLogger
    // - 通过平台抽象层输出
    // - 固定大小缓冲区 (256 字节)
    // - 时间戳 (毫秒)
    
    info!("no_std 环境日志");
    
    loop {}
}
```

## 性能优化

### 1. 编译时日志级别控制

```toml
[dependencies]
remdb = { path = "./remdb", features = ["log"] }

# 在 release 模式下，debug 和 trace 日志会被完全移除
# 零运行时开销
```

### 2. 条件日志

```rust
// 只在 debug 模式下编译
#[cfg(debug_assertions)]
{
    debug!("详细的调试信息");
}

// 只在特定 feature 启用时编译
#[cfg(feature = "verbose-logging")]
{
    trace!("非常详细的追踪信息");
}
```

### 3. 延迟格式化

```rust
// 避免在日志级别未启用时计算格式化参数
debug(|| format!("复杂计算结果: {}", expensive_computation()));
```

## 平台抽象层集成

no_std 环境下的日志输出通过平台抽象层实现：

```rust
// src/platform/mod.rs
pub trait Platform: Send + Sync {
    // ... 其他方法
    
    // 日志输出方法
    fn log_write(&self, buffer: *const u8, size: usize) -> Result<(), ()>;
}
```

### 自定义平台实现

```rust
use remdb::platform::{Platform, init_platform};

struct MyCustomPlatform;

impl Platform for MyCustomPlatform {
    fn log_write(&self, buffer: *const u8, size: usize) -> Result<(), ()> {
        // 实现自定义的日志输出
        // 例如：串口、RTT、ITM 等
        Ok(())
    }
    
    // ... 实现其他必需的方法
}

fn main() {
    init_platform(&MyCustomPlatform);
    init_logger();
    
    info!("使用自定义平台");
}
```

## 最佳实践

### 1. 日志级别选择

```rust
// trace: 非常详细的执行流程
trace!("开始处理记录 id={}", record_id);

// debug: 调试信息
debug!("查询条件: {:?}", query);

// info: 重要的业务事件
info!("用户登录: user_id={}", user_id);

// warn: 潜在问题
warn!("连接池接近上限: {}/{}", used, total);

// error: 错误情况
error!("数据库操作失败: {:?}", error);
```

### 2. 结构化日志

```rust
// 使用键值对格式
info!(
    user_id = user.id,
    action = "login",
    ip = request.ip(),
    "用户登录成功"
);
```

### 3. 错误处理

```rust
match result {
    Ok(data) => {
        info!("操作成功: {:?}", data);
    }
    Err(e) => {
        error!("操作失败: {:?}", e);
        // 处理错误
    }
}
```

## 迁移指南

### 从 println! 迁移

```rust
// 之前
println!("Debug: {}", value);

// 之后
debug!("Debug: {}", value);
```

### 从 eprintln! 迁移

```rust
// 之前
eprintln!("Error: {}", error);

// 之后
error!("Error: {}", error);
```

## 示例代码

完整的示例代码请参考：

- [examples/log_example.rs](file:///d:/workspace/remdb-server/remdb/examples/log_example.rs) - 基本使用示例
- [examples/low_power_mode.rs](file:///d:/workspace/remdb-server/remdb/examples/low_power_mode.rs) - 低功耗模式示例

## 故障排查

### 日志没有输出

1. 检查是否启用了 `log` feature
2. 确认调用了 `init_logger()`
3. 检查日志级别设置

### no_std 环境编译失败

1. 确保启用了 `baremetal` feature
2. 检查平台抽象层是否正确初始化
3. 确认 `heapless` 依赖可用

### 性能问题

1. 在 release 模式下编译
2. 减少日志级别
3. 使用条件编译禁用不必要的日志

## 总结

remdb 的日志系统提供了：

- ✅ 统一的 tracing API
- ✅ 完整的 no_std 支持
- ✅ 零成本抽象（release 模式）
- ✅ 灵活的平台适配
- ✅ 优秀的性能表现

通过合理使用日志系统，可以在开发和生产环境中获得良好的调试和监控能力。
