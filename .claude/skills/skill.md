# 🦀 大型Rust项目工程化技能清单

**适用场景**：多Crate工作空间、10万+行代码、10+人团队协作、Open Code联合开发   **版本**：Rust 2025 Edition / 2026 Nightly   **更新日期**：2026年2月

## 一、项目架构与模块化（规模化基石）

### ✅ 1.1 Cargo工作空间（Workspace）强制标准化

大型项目**必须**使用Cargo Workspace，严禁单Crate仓库。

```
# 根目录 Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/core",          # 核心领域模型
    "crates/infrastructure",# 基础设施（DB、MQ、Cache）
    "crates/application",   # 应用服务层
    "crates/interfaces",    # HTTP/GRPC/CLI接口层
    "crates/migration",    # 数据库迁移
]
default-members = ["crates/application"]
```

**Skill要点**：

- `resolver = "2"` 必须显式声明，避免特性冲突 
- 依赖统一管理：根`Cargo.toml`使用`workspace.dependencies`集中锁定版本
- Crate间通过`path`依赖，发布前再替换为版本号

### ✅ 1.2 模块边界与可见性控制

- **接口隔离**：每个Crate提供`lib.rs`作为外观（Facade），使用`pub use`重导出公共API，**禁止**其他Crate深度引用内部模块 
- **内部模块**：非公开模块设置为`pub(crate)`或私有，严禁`pub mod`泄漏实现细节
- **Feature Gate**：关键模块使用`#[cfg(feature = "...")]`控制编译，避免无关依赖膨胀

## 二、核心编码实践（防崩、防拖垮）

### ✅ 2.1 错误处理——生产级标准

**绝对禁止**：在库代码中使用`.unwrap()`、`.expect()`、`panic!`（除不可恢复故障）。  

**强制方案**：

```
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("无效输入: {0}")]
    InvalidInput(String),
    #[error("基础设施故障: {0}")]
    Infrastructure(#[from] std::io::Error),
    #[error("第三方服务超时")]
    Timeout,
}

pub type CoreResult<T> = Result<T, CoreError>;
```

**Skill要点**：

- 使用`thiserror`定义领域错误，使用`anyhow`仅在二进制/测试入口聚合 
- **上下文附加**：`.map_err(|e| CoreError::Infrastructure(e).context("读取配置文件失败"))`（需`anyhow::Context`）
- 错误传播必须使用`?`，**禁止**手动`match`传播

### ✅ 2.2 内存与所有权——性能生命线

**大型项目血泪教训**：滥用`clone()`导致内存爆炸，直接拒绝合并。  

**强制引用优先策略** ：

```
// ❌ 错误：Batch拥有所有权，导致Data被反复拷贝
struct Batch { msgs: Vec<String> }

// ✅ 正确：Batch存储引用，近乎零开销
struct Batch<'a> { msgs: Vec<&'a str> }
```

**Skill要点**：

- **引用生命周期**：必须掌握非侵入式生命周期标注，禁止用`clone()`逃避借用检查
- **Cow模式**：对“可能修改、可能不修改”的数据使用`std::borrow::Cow`
- **智能指针**：`Arc`仅在线程间共享时使用，单线程共享用`Rc`，**不可滥用** 
- **内存分析**：每周运行`cargo bench` + `heaptrack`检测内存泄漏

## 三、并发与异步（高吞吐保障）

### ✅ 3.1 异步运行时选型与调优

| 场景                  | 强制方案                   | 说明                            |
| --------------------- | -------------------------- | ------------------------------- |
| 高并发I/O（Web/网关） | `tokio` + 多线程调度器     | 线程数默认CPU核数×2，需压测调优 |
| 嵌入式/延迟极敏感     | `tokio` + `current_thread` | 单线程，避免跨核同步开销        |
| 轻量工具/教学         | `async-std`                | **大型项目不推荐**，生态滞后    |

**Skill要点** ：

- **CPU密集任务**：必须使用`tokio::task::spawn_blocking`移交线程池，**严禁**直接在`async`函数内计算斐波那契
- **阻塞禁忌**：禁止`std::thread::sleep`、同步`std::fs`、同步锁；一律替换为`tokio::time::sleep`、`tokio::fs`
- **限流**：使用`tokio::sync::Semaphore`限制并发数据库连接/文件句柄

### ✅ 3.2 异步取消与优雅关闭（2025+必备）

大型服务必须支持**秒级优雅退出**，防止重启时大量502。

```
use tokio_util::sync::CancellationToken;

let token = CancellationToken::new();
let child_token = token.child_token();

tokio::select! {
    _ = worker.run(child_token) => {},
    _ = shutdown_signal() => {
        token.cancel();  // 广播取消信号
    }
}
```

**Skill要点**：

- 每个长任务必须持有`CancellationToken`，定期检查`.is_cancelled()`
- 资源释放（DB连接、文件锁）必须实现在`Drop`或使用`ScopeGuard`

## 四、测试与质量门禁（CI/CD硬性关卡）

### ✅ 4.1 三层测试体系

1. **单元测试**：模块内`#[cfg(test)]`，mock外部依赖
2. **集成测试**：`tests/`目录，测试Crate间的公共API
3. **端到端测试**：独立测试工程，调用真实服务（每日构建）

**强制覆盖** ：

- 文档测试：`cargo test --doc`，所有公共API必须包含`# Examples`
- 模糊测试：`cargo fuzz`，对解析类模块必须添加
- 基准测试：`criterion`，性能敏感函数必须有基准用例

### ✅ 4.2 CI质量门禁（拒绝脏代码）

```
# GitHub Actions / GitLab CI 强制步骤
- run: cargo fmt --check
- run: cargo clippy --workspace -- -D warnings
- run: cargo test --workspace --locked
- run: cargo deny check  # 许可证与依赖安全
- run: cargo audit      # 漏洞扫描
- run: cargo miri test  # 检测unsafe UB（夜间工具链）
```

**Skill要点** ：

- **硬性红线**：Clippy警告即构建失败，不允许任何`allow`绕过（除非核心团队评审）
- **依赖审查**：每周执行`cargo outdated`，依赖版本落后超过6个月需升级

## 五、性能工程（从“能用”到“扛住”）

### ✅ 5.1 编译期优化配置

```
[profile.release]
opt-level = 3          # 强优化
lto = "fat"           # 全链接时优化，必开
codegen-units = 1     # 合并代码生成单元，提升运行时性能
strip = "symbols"     # 去除符号表，减镜像体积
```

**Skill要点** ：   大型项目**必须**设置`lto = "fat"`和`codegen-units = 1`，性能提升可达20%以上。代价是编译时间增加，需配合`--timings`分析瓶颈。

### ✅ 5.2 性能分析与火焰图

**日常工具链** ：

```
# 性能采样
sudo perf record -g -F 99 target/release/your_app
sudo perf script | stackcollapse-perf | flamegraph.pl > flame.svg

# Rust原生工具
cargo install flamegraph
cargo flamegraph --bin your_app
```

**Skill要点**：   性能优化**不准靠猜**。每次重大变更必须附优化前后的火焰图对比。

## 六、可观测性与运维（线上不裸奔）

### ✅ 6.1 结构化日志（弃用`log`）

**强制**：使用`tracing`替代`log` 。

```
use tracing::{info, error, instrument};

#[instrument(skip(password))]
pub fn login(username: &str, password: &str) -> CoreResult<()> {
    info!("用户登录尝试");  // 自动附加span字段（username）
    // ...
}
```

**Skill要点**：

- 每个异步入口函数必须标注`#[instrument]`，自动追踪调用链
- 日志级别规范：`ERROR`仅用于需要值班处理的故障，业务异常使用`WARN`

### ✅ 6.2 指标与健康检查

- **指标暴露**：使用`metrics` + `prometheus`，导出QPS、延迟、错误率、内存
- **存活/就绪探针**：实现`GET /health`和`GET /ready`，K8s必备
- **panic兜底**：`std::panic::set_hook`，将panic信息写入独立文件并上报

## 七、Open Code协作规范（团队生存指南）

### ✅ 7.1 代码风格强制一致

- `.rustfmt.toml` 团队锁定，禁止个人自定义
- 编辑器配置：`rust-analyzer` + `VS Code`，共享`.vscode/extensions.json`和`settings.json`

### ✅ 7.2 文档即代码

- 所有公共API必须有文档示例（文档测试强制通过）
- **架构决策记录（ADR）**：每个重大设计必须在`docs/adr/`中记录，采用Markdown格式，包含上下文、决策、后果

### ✅ 7.3 审查清单（Review Checklist）

每个PR必须自检：

-  是否引入`unwrap()`？是否有不可恢复的充分理由？
-  是否添加了单元测试/集成测试？
-  性能敏感代码是否附加了基准测试数据？
-  `unsafe`是否经过2人以上安全评审并添加`// SAFETY:`注释？
-  依赖是否经过`cargo deny`检查？

## 📦 技能速查表（随身版）

| 维度       | 核心工具/模式                | 关键命令                     |
| ---------- | ---------------------------- | ---------------------------- |
| 项目管理   | Cargo Workspace              | `cargo new --lib crates/xxx` |
| 错误处理   | `thiserror` + `anyhow`       | `#[derive(Error)]`           |
| 异步运行时 | `tokio`                      | `#[tokio::main]`             |
| 结构化并发 | `CancellationToken`          | `token.cancel()`             |
| 并行计算   | `rayon` / `spawn_blocking`   | `.par_iter()`                |
| 依赖安全   | `cargo deny` / `cargo audit` | `cargo deny check`           |
| 性能分析   | `flamegraph` / `criterion`   | `cargo flamegraph`           |
| unsafe检测 | `miri`                       | `cargo miri test`            |
| 日志追踪   | `tracing`                    | `#[instrument]`              |

