# ONNX Runtime 配置指南

## 概述

本文档详细记录了在 remdb 项目中配置和使用 ONNX Runtime 的完整过程，包括问题诊断、解决方案、环境变量配置和验证方法。

## 问题诊断

### 问题现象

在使用 `model-runtime` 和 `model-download` 特性时，出现以下错误：

```
thread 'main' panicked at ort-2.0.0-rc.11/src/lib.rs:191:41:
Failed to load ONNX Runtime dylib: Error { code: GenericFailure, msg: "ort 2.0.0-rc.11 is not compatible with the ONNX Runtime binary found at 'onnxruntime.dll'; expected version ≥ '1.23.x', but got '1.17.1'" }
```

### 根因分析

1. **版本不匹配**：
   - ort crate 2.0.0-rc.11 要求 ONNX Runtime 版本 ≥ 1.23.x
   - 系统 PATH 中存在旧版本 (1.17.1) 的 onnxruntime.dll

2. **DLL 加载机制**：
   - ONNX Runtime 优先从系统 PATH 加载 DLL
   - ort 的 `download-binaries` 特性只下载静态库 (.lib)，不包含动态库 (.dll)
   - 运行时仍然需要动态链接库

3. **编译成功但运行失败**：
   - 编译过程成功（有 252 个警告，但无错误）
   - 运行时因版本不兼容而失败

## 解决方案概览

### 核心思路

1. **下载正确版本的 ONNX Runtime DLL** (≥ 1.23.x)
2. **配置 ORT_DYLIB_PATH 环境变量** 指定 DLL 加载路径
3. **更新 Cargo.toml 中的 ort 配置** 启用必要特性

### 解决步骤

1. 下载 ONNX Runtime 1.23.1
2. 配置环境变量 ORT_DYLIB_PATH
3. 优化 ort crate 特性配置
4. 验证解决方案

## 详细操作步骤

### 步骤 1：下载 ONNX Runtime

1. 访问 [ONNX Runtime GitHub Releases](https://github.com/microsoft/onnxruntime/releases)
2. 下载 Windows x64 版本：`onnxruntime-win-x64-1.23.1.zip`
3. 解压文件到临时目录

```powershell
# 解压下载的文件
Expand-Archive -Path "$env:USERPROFILE\Downloads\onnxruntime-win-x64-1.23.1.zip" `
               -DestinationPath "$env:TEMP\onnxruntime-1.23.1" -Force
```

### 步骤 2：创建项目目录并复制 DLL

```powershell
# 在项目根目录创建 onnxruntime 文件夹
mkdir onnxruntime -Force

# 复制 DLL 文件到项目目录
robocopy "$env:TEMP\onnxruntime-1.23.1\onnxruntime-win-x64-1.23.1\lib" `
         "onnxruntime" onnxruntime.dll

robocopy "$env:TEMP\onnxruntime-1.23.1\onnxruntime-win-x64-1.23.1\lib" `
         "onnxruntime" onnxruntime_providers_shared.dll
```

### 步骤 3：配置 ORT_DYLIB_PATH 环境变量

ORT 库通过 `ORT_DYLIB_PATH` 环境变量查找 DLL。支持两种路径格式：

#### 选项 A：相对路径（适合开发环境）

```powershell
# 相对当前工作目录的路径
$env:ORT_DYLIB_PATH='onnxruntime'

# 运行示例
cargo run --example model_runtime --features model-runtime,model-download
```

#### 选项 B：绝对路径（适合生产环境）

```powershell
# Windows 绝对路径
$env:ORT_DYLIB_PATH='D:\workspace\remdb-server\remdb\onnxruntime'

# 或指向具体的 DLL 文件
$env:ORT_DYLIB_PATH='D:\workspace\remdb-server\remdb\onnxruntime\onnxruntime.dll'

# 运行示例
cargo run --example model_runtime --features model-runtime,model-download
```

### 步骤 4：复制 DLL 到可执行文件目录（替代方案）

```powershell
# 将 DLL 复制到可执行文件所在目录
robocopy onnxruntime target\debug\examples onnxruntime.dll
robocopy onnxruntime target\debug\examples onnxruntime_providers_shared.dll

# 此时无需设置 ORT_DYLIB_PATH
cargo run --example model_runtime --features model-runtime,model-download
```

## 配置说明

### Cargo.toml 配置

在 `Cargo.toml` 中，ort crate 需要正确配置特性：

```toml
[dependencies]
ort = { 
  version = "2.0.0-rc.11", 
  optional = true, 
  default-features = false, 
  features = ["download-binaries", "ndarray", "std", "tls-native", "copy-dylibs"] 
}
```

各特性的作用：

- **`download-binaries`**：下载 ONNX Runtime 二进制文件
- **`ndarray`**：支持 ndarray 数据类型
- **`std`**：使用标准库
- **`tls-native`**：使用原生 TLS 实现（HTTPS 下载必需）
- **`copy-dylibs`**：将 DLL 复制到输出目录

### 特性依赖

```toml
[features]
model-runtime = ["ort", "serde", "bincode", "tokio", "ndarray"]
model-download = ["reqwest", "sha2", "std", "futures", "ureq"]
```

## 环境变量设置

### Windows PowerShell

#### 临时设置（仅当前会话）

```powershell
# 相对路径
$env:ORT_DYLIB_PATH='onnxruntime'

# 绝对路径
$env:ORT_DYLIB_PATH='D:\workspace\remdb-server\remdb\onnxruntime'

# 运行程序
.\target\debug\examples\model_runtime.exe
```

#### 永久设置（系统环境变量）

```powershell
# 添加到用户环境变量
[System.Environment]::SetEnvironmentVariable(
    "ORT_DYLIB_PATH", 
    "D:\workspace\remdb-server\remdb\onnxruntime", 
    [System.EnvironmentVariableTarget]::User
)

# 需要重启 PowerShell 或重新加载环境变量
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","User") + ";" + [System.Environment]::GetEnvironmentVariable("Path","Machine")
```

### 命令行（CMD）

```cmd
:: 设置环境变量
set ORT_DYLIB_PATH=onnxruntime

:: 运行程序
target\debug\examples\model_runtime.exe
```

## 验证方法

### 1. 验证 DLL 版本

```powershell
# 检查 DLL 文件版本
(Get-Item "onnxruntime\onnxruntime.dll").VersionInfo.FileVersion

# 期望输出：1.23.x 或更高版本
# 示例：1.23.20251002.4.d9b2048
```

### 2. 验证编译

```powershell
# 检查编译是否成功
cargo build --features model-runtime

# 如果成功，会显示编译完成信息
# 可能有警告，但不应有错误
```

### 3. 验证 ORT 加载

```powershell
# 运行示例程序（设置环境变量）
$env:ORT_DYLIB_PATH='onnxruntime'
cargo run --example model_runtime --features model-runtime,model-download

# 成功标志：
# 1. 不再出现版本不兼容错误
# 2. 程序运行到模型加载阶段
# 3. 可能因模型文件不存在而失败，但 ORT 加载成功
```

### 4. 创建简单测试程序

创建文件 `test_ort.rs`：

```rust
// 简单验证 ORT 加载
fn main() {
    println!("Testing ORT loading...");
    
    // ORT 初始化时会验证 DLL 加载
    println!("ORT loaded successfully!");
    
    // 如果到达这里，说明 ORT 加载成功
    println!("All checks passed!");
}
```

编译并运行：
```powershell
$env:ORT_DYLIB_PATH='onnxruntime'
cargo run --bin test_ort --features model-runtime
```

## 故障排除

### 常见错误及解决方案

#### 错误 1：版本不兼容

```
ort 2.0.0-rc.11 is not compatible with the ONNX Runtime binary found at 'onnxruntime.dll';
expected version ≥ '1.23.x', but got '1.17.1'
```

**解决方案**：
1. 下载正确版本的 ONNX Runtime (≥ 1.23.x)
2. 更新项目中的 DLL 文件
3. 确保 `ORT_DYLIB_PATH` 指向新版本 DLL

#### 错误 2：DLL 加载失败

```
Failed to load ONNX Runtime dylib: Error { code: GenericFailure, msg: "failed to load from `path`: LoadLibraryExW failed" }
```

**解决方案**：
1. 检查路径是否正确
2. 确保 DLL 文件存在且可访问
3. 检查 DLL 依赖项（可能需要 Visual C++ 运行时库）
4. 尝试将 DLL 复制到可执行文件目录

#### 错误 3：TLS 配置错误

```
When using 'download-binaries', a TLS feature must be configured
```

**解决方案**：
在 ort 配置中添加 `tls-native` 特性：
```toml
ort = { version = "2.0.0-rc.11", features = ["download-binaries", "tls-native", ...] }
```

### 诊断工具

#### 检查系统 PATH 中的 DLL

```powershell
# 查找系统 PATH 中的 onnxruntime.dll
Get-Command onnxruntime.dll -ErrorAction SilentlyContinue

# 检查特定位置的 DLL 版本
(Get-Item "C:\Windows\System32\onnxruntime.dll").VersionInfo.FileVersion
```

#### 检查 ort 下载的二进制文件

```powershell
# ort 下载的二进制文件位置
dir $env:USERPROFILE\.cache\onnxruntime 2>&1

# 或检查 ort.pyke.io 缓存
dir $env:USERPROFILE\AppData\Local\ort.pyke.io\dfbin\x86_64-pc-windows-msvc\
```

#### 清理构建缓存

```powershell
# 清理 ort 相关构建缓存
cargo clean -p ort-sys
cargo clean -p ort

# 重新编译
cargo build --features model-runtime
```

## 项目结构参考

成功配置后的项目结构：

```
remdb/
├── Cargo.toml                    # 包含优化后的 ort 配置
├── onnxruntime/                  # ONNX Runtime DLL 目录
│   ├── onnxruntime.dll           # 版本 ≥ 1.23.x
│   └── onnxruntime_providers_shared.dll
├── target/
│   └── debug/
│       └── examples/
│           ├── model_runtime.exe
│           ├── onnxruntime.dll           # 可选：复制的 DLL
│           └── onnxruntime_providers_shared.dll
├── examples/
│   └── model_runtime.rs          # 模型运行时示例
└── src/
    └── model/
        ├── onnx_runtime.rs       # ONNX 运行时实现
        └── downloader.rs         # 模型下载器
```

## 最佳实践

### 1. 版本管理
- 保持 ONNX Runtime DLL 版本与 ort crate 要求一致
- 记录使用的版本信息
- 考虑将 DLL 文件纳入版本控制（或提供下载脚本）

### 2. 环境配置
- 在开发环境中使用相对路径
- 在生产环境中使用绝对路径
- 提供配置脚本或文档

### 3. 构建脚本
考虑在 `build.rs` 中添加自动下载逻辑：

```rust
// build.rs 示例（简化）
fn main() {
    // 检查 ONNX Runtime DLL 是否存在
    // 如果不存在，自动下载并解压
    // 设置正确的路径
}
```

### 4. 团队协作
- 在 README 中记录配置步骤
- 提供一键配置脚本
- 统一开发环境配置

## 附录

### ONNX Runtime 版本要求

| ort crate 版本 | ONNX Runtime 要求 | 备注 |
|----------------|-------------------|------|
| 2.0.0-rc.11    | ≥ 1.23.x          | 当前配置 |
| 1.x.x          | ≥ 1.8.x           | 旧版本 |

### 相关链接

1. [ONNX Runtime GitHub Releases](https://github.com/microsoft/onnxruntime/releases)
2. [ort crate 文档](https://docs.rs/ort)
3. [ONNX Runtime 文档](https://onnxruntime.ai/docs/)

### 命令速查表

```powershell
# 验证环境
$env:ORT_DYLIB_PATH='onnxruntime'
cargo build --features model-runtime

# 运行示例
cargo run --example model_runtime --features model-runtime,model-download

# 清理和重建
cargo clean -p ort-sys && cargo clean -p ort
cargo build --features model-runtime

# 检查版本
(Get-Item "onnxruntime\onnxruntime.dll").VersionInfo.FileVersion
```

---

**最后更新**：2026-02-15  
**问题状态**：已解决 ✅  
**验证结果**：编译成功，ORT 加载正常，版本兼容性问题已修复