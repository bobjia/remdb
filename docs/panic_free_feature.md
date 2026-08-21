# remdb Panic-Free Feature Specification

[TOC]

## 目标

将 remdb 库代码（`src/`）中所有可能导致 panic 的代码路径消除，使库在任何情况下都返回 `Result<T, RemDbError>` 而非 panic。特别关注 `no_std` / baremetal 场景，因为 panic 在嵌入式系统上通常意味着不可恢复的系统崩溃。

## 当前状态

| 指标 | 数量 |
|------|------|
| `.unwrap()` 调用 | 257 |
| `.expect()` 调用 | 25+ |
| 显式 `panic!()` | 24 |
| **总计 panic 点** | **~306** |

## 1. 分类与应对策略

### 1.1 Platform 未初始化（20 处 panic）

**位置**: `src/platform/mod.rs`

**现状**:
```rust
pub fn get_timestamp() -> u64 {
    if let Some(platform) = PLATFORM.get() {
        platform.get_timestamp()
    } else {
        panic!("Platform not initialized")
    }
}
```

**方案对比**:

| 方案 | 描述 | 优点 | 缺点 | 推荐度 |
|------|------|------|------|--------|
| A. 返回 `Result` | 改为 `fn get_timestamp() -> Result<u64, RemDbError>` | 安全、可恢复 | 破坏性 API 变更 | ⭐⭐⭐⭐⭐ |
| B. 默认值回退 | 返回 0 并记录错误 | 无 API 变更 | 静默失败，难以调试 | ⭐⭐ |
| C. 初始化守卫 | 编译期保证 init 被调用 | 零运行时开销 | 需要 proc macro / type-state 模式 | ⭐⭐⭐ |
| D. Debug 断言 | `debug_assert!` + 回退值 | 仅 debug 检查 | release 下静默失败 | ⭐⭐ |

**推荐**: **方案 A** — 所有 platform 函数返回 `Result`。增加 `RemDbError::PlatformNotInitialized` 变体。

**影响范围**: 约 20 个函数，所有调用点需要适配（估计 50+ 处调用）。

---

### 1.2 锁中毒 / 锁失败（30+ 处 unwrap）

**位置**: `src/time_series/`, `src/index/`, `src/lib.rs`, `src/index/builder.rs`

**现状**:
```rust
let mut time_index = self.time_index.write().unwrap();
let partitions_guard = table.partitions.lock().unwrap();
```

**根因分析**:
- `std::sync::Mutex::lock()` 返回 `LockResult`，当其他线程 panic 时返回 `PoisonError`
- `std::sync::RwLock` 同理
- 代码中所有 `.unwrap()` 假设锁永远不会中毒

**方案对比**:

| 方案 | 描述 | 优点 | 缺点 | 推荐度 |
|------|------|------|------|--------|
| A. 自定义 SpinLock | 替换 std Mutex/RwLock，不中毒 | 无中毒概念，适合 no_std | 需要实现，且要处理并发 | ⭐⭐⭐⭐⭐ |
| B. 安全包装器 | `lock().unwrap_or_else(\|e\| e.into_inner())` | 最小改动 | 可能掩盖数据损坏 | ⭐⭐⭐⭐ |
| C. 返回错误 | 每次 lock 返回 `Result` | 最安全 | 代码冗长 | ⭐⭐⭐ |
| D. `try_lock()` + 重试 | 非阻塞 + 有限重试 | 无死锁风险 | 可能饥饿 | ⭐⭐ |

**推荐**:
- **no_std / baremetal**: 方案 A — 自定义 `SpinMutex` / `SpinRwLock`（项目已有 `spin_lock` / `spin_unlock` 原语）
- **std 环境**: 方案 B — 安全包装器，快速见效

**新增类型**:
```rust
// src/sync.rs (新模块)
pub struct SpinMutex<T> { /* ... */ }
impl<T> SpinMutex<T> {
    pub fn lock(&self) -> SpinLockGuard<T>;  // 不返回 Result，不中毒
    pub fn try_lock(&self) -> Option<SpinLockGuard<T>>;
}
```

---

### 1.3 内存分配失败（15+ 处 expect）

**位置**: `src/index.rs`, `src/index/builder.rs`

**现状**:
```rust
let mut new_node = self.allocate_node()
    .expect("Out of memory for B-Tree node");
let root = self.root.expect("Root node unexpectedly None");
```

**方案对比**:

| 方案 | 描述 | 优点 | 缺点 | 推荐度 |
|------|------|------|------|--------|
| A. 返回 `Result` | 传播分配错误 | 安全、可恢复 | 大量函数签名变更 | ⭐⭐⭐⭐⭐ |
| B. 预分配 | 初始化时预留，运行时永不失败 | 零运行时开销 | 灵活性差，浪费内存 | ⭐⭐⭐ |
| C. 回退策略 | 分配失败时降级（如全表扫描） | 自适应 | 逻辑复杂 | ⭐⭐ |

**推荐**: **方案 A** — 所有分配点返回 `Result<_, RemDbError::OutOfMemory>`。

**具体变更**:
- `allocate_node()` 返回 `Result<NodePtr, RemDbError>`
- `root` 访问改为 `self.root.ok_or(RemDbError::InternalError("Root node missing"))?`
- 向上传播直到可处理层级

---

### 1.4 数组切片转换（10+ 处 unwrap）

**位置**: `src/time_series/compression.rs`

**现状**:
```rust
let delta = u64::from_le_bytes(delta_bytes.try_into().unwrap());
```

**分析**: 这些转换来自固定大小的缓冲区切片，理论上不会失败。但 `unwrap()` 仍然存在 panic 风险（如缓冲区损坏）。

**方案对比**:

| 方案 | 描述 | 优点 | 缺点 | 推荐度 |
|------|------|------|------|--------|
| A. 切片模式 | `let [b0,..,b7] = *delta_bytes else { return Err(...) }` | 安全、零开销 | 需要 Rust 1.65+ | ⭐⭐⭐⭐⭐ |
| B. 错误传播 | `.try_into().map_err(\|_\| RemDbError::InvalidData)?` | 安全 | 稍冗长 | ⭐⭐⭐⭐ |
| C. unsafe 指针 | 直接 `*(ptr as *const [u8; 8])` | 零开销 | unsafe | ⭐ |

**推荐**: **方案 A**（切片模式）用于 Rust 1.65+，否则 **方案 B**。

**辅助宏**:
```rust
macro_rules! try_array {
    ($slice:expr, $ty:ty, $err:expr) => {
        <$ty>::try_from($slice).map_err(|_| $err)?
    };
}
```

---

### 1.5 可变大小类型取 size（5 处 panic）

**位置**: `src/types.rs`

**现状**:
```rust
pub fn size(&self) -> usize {
    match self {
        // ...
        DataType::VarChar => panic!("VarChar size is variable at compile time"),
        DataType::Text => panic!("Text size is variable at compile time"),
        DataType::Vector => panic!("Vector size depends on dimension at runtime"),
        DataType::Json => panic!("Json size is variable at runtime"),
        // ...
    }
}
```

**方案对比**:

| 方案 | 描述 | 优点 | 缺点 | 推荐度 |
|------|------|------|------|--------|
| A. 返回 `Option<usize>` | `None` 表示可变大小 | 安全、清晰 | 所有调用点需适配 | ⭐⭐⭐⭐⭐ |
| B. 返回 0 | 哨兵值 | 无 API 变更 | 语义模糊，0 可能被误用 | ⭐⭐ |
| C. 拆分为两个方法 | `fixed_size() -> Option<usize>` + `is_variable_size() -> bool` | 向后兼容 | 增加 API 面积 | ⭐⭐⭐⭐ |

**推荐**: **方案 A** — `fn size(&self) -> Option<usize>`。新增 `fn fixed_size(&self) -> Option<usize>` 别名。

**迁移**:
```rust
// 旧代码
let s = data_type.size();

// 新代码
let s = data_type.size().ok_or(RemDbError::VariableSizeType)?;
```

---

### 1.6 SQL 解析/执行器 panic（2 处）

**位置**: `src/sql/query_parser.rs` (L4403), `src/sql/query_executor.rs` (L2075)

**现状**:
```rust
// query_parser.rs
_ => panic!("Expected CreateTable query"),

// query_executor.rs
panic!("Field not found: {}", field_name_part);
```

**推荐**: 直接替换为 `Err(RemDbError::ParseError(...))` 和 `Err(RemDbError::FieldNotFound(...))`。这些函数已经返回 `Result`，改动最小。

---

### 1.7 内存池指针未找到（1 处 panic）

**位置**: `src/memory/pool.rs`

**现状**:
```rust
panic!("Pointer not found in any memory pool");
```

**推荐**: 改为返回 `Err(RemDbError::InvalidPointer)`。这是 `free()` 方法，应返回 `Result`。

---

### 1.8 Worker 协议错误（1 处 panic）

**位置**: `src/model/worker_protocol.rs`

**现状**:
```rust
_ => panic!("Wrong request type"),
```

**推荐**: 改为 `Err(RemDbError::ProtocolError(...))`。

---

### 1.9 其余 ~170 处 unwrap（lib.rs, c_api.rs, pubsub 等）

**策略**: 分三档处理：

| 档位 | 类型 | 处理方法 | 数量估计 |
|------|------|----------|----------|
| **安全 unwrap** | 逻辑上不可能失败的 `Option`/`Result` | 添加 `// SAFETY:` 注释 + `#[allow(clippy::unwrap_used)]` | ~30 |
| **可失败 unwrap** | 理论上可能失败 | 改为 `?` 或 `.ok_or()?` | ~120 |
| **需要重构** | 需要改变控制流 | 重构为 `match` 或 `if let` | ~20 |

---

## 2. 新增错误类型

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum RemDbError {
    // 现有变体...

    // 新增
    /// 平台未初始化
    PlatformNotInitialized,
    /// 内存不足
    OutOfMemory,
    /// 锁错误
    LockError,
    /// 无效指针
    InvalidPointer,
    /// 无效数据
    InvalidData(&'static str),
    /// 可变大小类型
    VariableSizeType,
    /// 字段未找到
    FieldNotFound(String),
    /// 协议错误
    ProtocolError(String),
    /// 内部错误
    InternalError(&'static str),
    /// 意外的 None 值
    UnexpectedNone(&'static str),
}
```

---

## 3. 实施路线图

| 阶段 | 内容 | 周期 | 风险 |
|------|------|------|------|
| **Phase 1: 基础** | ① 新增错误变体 ② 修复 platform 模块 (20 处) ③ 修复 types.rs (5 处) ④ 修复 memory/pool.rs (1 处) | 1-2 周 | 低 |
| **Phase 2: 核心** | ① 自定义 SpinLock ② 替换所有锁 unwrap (30+ 处) ③ 修复 index 分配 expect (15+ 处) ④ 修复 compression unwrap (10+ 处) | 2-3 周 | 中 |
| **Phase 3: 清理** | ① 修复 SQL parser/executor (2 处) ② 修复 worker protocol (1 处) ③ 修复 lib.rs 剩余 unwrap (~100 处) ④ 修复 c_api.rs (~38 处) | 2-3 周 | 中 |
| **Phase 4: 加固** | ① 添加 `#![deny(clippy::unwrap_used)]` ② CI 检查 ③ no_std 测试 ④ 压力测试 | 1-2 周 | 低 |

---

## 4. 成功标准

1. **零 panic**: `src/` 中 0 个 `.unwrap()`、`.expect()`、`panic!()`（除标注 `#[allow]` 的不可失败点）
2. **CI 强制执行**: 新代码禁止引入 unwrap，clippy 检查不通过则 CI 失败
3. **no_std 兼容**: baremetal 目标编译通过，所有平台函数返回 `Result`
4. **性能不退化**: 基准测试波动 < 5%
5. **向后兼容**: 提供 deprecated wrapper 过渡 1-2 个版本

---

## 5. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 大量 API 变更导致下游代码 break | 使用 `#[deprecated]` 标记旧 API，保留 2 个版本 |
| 错误处理路径性能下降 | 错误路径标记 `#[cold]`，热路径不变 |
| 自定义 SpinLock 的正确性 | 充分测试 + miri 检查 + code review |
| no_std 下错误类型不可用 | 使用 `core::result::Result`，错误类型实现 `Debug` + `PartialEq` |

---

## 6. 辅助宏

为减少重复代码，提供以下辅助宏：

```rust
/// 安全地获取锁，失败时返回错误
macro_rules! try_lock {
    ($lock:expr) => {{
        match $lock.lock() {
            Ok(guard) => guard,
            Err(_) => return Err(RemDbError::LockError),
        }
    }};
}

/// 安全地解包 Option，失败时返回错误
macro_rules! try_some {
    ($opt:expr, $msg:expr) => {{
        match $opt {
            Some(v) => v,
            None => return Err(RemDbError::UnexpectedNone($msg)),
        }
    }};
}

/// 安全地转换切片为定长数组
macro_rules! try_array {
    ($slice:expr, $ty:ty) => {{
        <$ty>::try_from($slice).map_err(|_| RemDbError::InvalidData("array conversion failed"))?
    }};
}
```

---

## 7. 附录：完整 panic 点清单

### 7.1 显式 `panic!()` 调用

| 文件 | 行号 | 内容 |
|------|------|------|
| `src/platform/mod.rs` | 170-323 | 20 处 `panic!("Platform not initialized")` |
| `src/types.rs` | 387-391 | 5 处变量大小 panic |
| `src/memory/pool.rs` | 149 | `panic!("Pointer not found in any memory pool")` |
| `src/sql/query_executor.rs` | 2075 | `panic!("Field not found: {}", ...)` |
| `src/sql/query_parser.rs` | 4403 | `panic!("Expected CreateTable query")` |
| `src/model/worker_protocol.rs` | 198 | `panic!("Wrong request type")` |

### 7.2 `.expect()` 调用（部分）

| 文件 | 数量 | 典型内容 |
|------|------|----------|
| `src/index.rs` | 15+ | `expect("Out of memory for B-Tree node")` |
| `src/index/builder.rs` | 5+ | `expect("Failed to spawn thread")` |
| `src/platform/posix.rs` | 2 | `expect("Time went backwards")` |
| `src/types.rs` | 2 | `expect("Time went backwards")` |
| `src/memory/allocator.rs` | 1 | `expect("Failed to update memory pool")` |

### 7.3 `.unwrap()` 调用（按模块）

| 模块 | 数量 | 主要模式 |
|------|------|----------|
| `src/lib.rs` | ~50 | 锁、表访问、索引访问 |
| `src/sql/query_executor.rs` | ~128 | 各种操作 unwrap |
| `src/sql/query_parser.rs` | ~74 | 解析结果 unwrap |
| `src/c_api.rs` | ~38 | FFI 边界 unwrap |
| `src/time_series/` | ~30 | 锁、分区访问 |
| `src/pubsub/subscriber.rs` | ~32 | 订阅操作 unwrap |
| `src/index/builder.rs` | ~10 | 构建状态 unwrap |
| `src/index/hnsw.rs` | ~8 | 向量索引操作 |
| `src/time_series/compression.rs` | ~10 | 数组转换 |