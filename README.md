# remdb - 嵌入式内存数据库

[English Version](./README_EN.md)

remdb是一个轻量级的嵌入式内存数据库，专为资源受限的嵌入式系统设计，支持no_std环境，具有可预测的内存使用和高性能。

## 主要功能

- **内存表存储**：高效的内存表实现，支持插入、删除、查询和遍历操作
- **索引机制**：
  - 基于哈希的主键索引，提供O(1)的查询性能
  - 基于有序数组的辅助索引，支持范围查询
- **事务支持**：完整的ACID事务支持，包括：
  - 原子性：事务要么全部提交，要么全部回滚
  - 一致性：确保数据的完整性和正确性
  - 隔离性：支持多种隔离级别（未提交读、提交读、可重复读、串行化）
  - 持久性：通过预写日志（WAL）确保数据持久化
  - 支持记录级锁（共享锁和排他锁）
  - 支持事务日志和崩溃恢复
- **内存管理**：
  - 静态内存分配器，无动态内存分配
  - 固定大小块内存池，支持高效的内存管理
- **平台抽象层**：支持POSIX和裸机环境
- **编译时配置**：使用宏实现表和数据库的编译时配置，优化性能
- **低功耗模式**：
  - 支持进入和退出低功耗模式
  - 低功耗模式下优化内存使用
  - 减少事务日志写入频率，降低磁盘I/O
  - 当记录数超过限制时，自动覆盖最旧的记录

## 技术特点

- **零外部依赖**：不依赖任何外部库，支持no_std环境
- **静态内存分配**：可预测的内存使用，适合资源受限的嵌入式系统
- **编译时优化**：通过宏实现编译时配置，减少运行时开销
- **多平台支持**：支持POSIX和裸机环境
- **类型安全**：使用Rust的类型系统确保数据安全
- **高效的同步机制**：实现了自旋锁同步机制，适合多线程环境

## 快速开始

### 安装

将remdb添加到你的Cargo.toml文件中：

```toml
[dependencies]
remdb = { path = "./remdb", default-features = false }

# 可选特性
# features = ["std", "posix"]
```

### 基本使用示例

```rust
#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::alloc::Layout;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

// 定义表结构
remdb::table!(
    users,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(32), // 32字节定长字符串
        age: i8,
        active: bool,
        created_at: u64
    }
);

// 定义数据库配置
remdb::database!(
    tables: [users]
);

// 内存分配错误处理
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Allocation error: {:?}", layout);
}

fn main() {
    unsafe {
        // 获取数据库配置
        let config = database!(tables: [users]);
        
        // 初始化内存分配器
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
        platform::init_platform(platform::posix::get_posix_platform());
        
        // 计算所需内存大小
        let table_size = MemoryTable::calculate_memory_size(config.tables[0]);
        let primary_index_size = PrimaryIndex::calculate_memory_size(
            config.tables[0],
            128, // 哈希表大小
            100  // 最大索引项数量
        );
        let secondary_index_size = SecondaryIndex::calculate_memory_size(100);
        
        // 分配内存
        let table_ptr = memory::allocator::alloc(table_size).unwrap().as_ptr() as *mut u8;
        let status_ptr = memory::allocator::alloc(
            core::mem::size_of::<types::RecordHeader>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut types::RecordHeader;
        
        let hash_table_ptr = memory::allocator::alloc(
            128 * core::mem::size_of::<Option<NonNull<index::PrimaryIndexItem>>>()
        ).unwrap().as_ptr() as *mut Option<NonNull<index::PrimaryIndexItem>>;
        
        let primary_index_items_ptr = memory::allocator::alloc(
            100 * core::mem::size_of::<index::PrimaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::PrimaryIndexItem;
        
        let secondary_index_items_ptr = memory::allocator::alloc(
            100 * core::mem::size_of::<index::SecondaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::SecondaryIndexItem;
        
        // 创建表和索引
        let mut table = MemoryTable::new(config.tables[0], table_ptr, status_ptr);
        let mut primary_index = PrimaryIndex::new(
            config.tables[0],
            hash_table_ptr,
            primary_index_items_ptr,
            128,
            100
        );
        let mut secondary_index = SecondaryIndex::new(config.tables[0], secondary_index_items_ptr, 100);
        
        // 初始化表和索引数组
        static mut TABLES: [Option<MemoryTable>; 1] = [None; 1];
        static mut PRIMARY_INDICES: [Option<PrimaryIndex>; 1] = [None; 1];
        static mut SECONDARY_INDICES: [Option<SecondaryIndex>; 1] = [None; 1];
        
        TABLES[0] = Some(table);
        PRIMARY_INDICES[0] = Some(primary_index);
        SECONDARY_INDICES[0] = Some(secondary_index);
        
        // 初始化全局数据库
        let db = init_global_db(
            config,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        ).unwrap();
        
        // 使用数据库...
    }
}
```

### 低功耗模式使用示例

```rust
// 定义支持低功耗模式的数据库
remdb::database!(
    TEST_DB,
    tables: [
        TEST_TABLE
    ],
    low_power: true,
    low_power_max_records: 100
);

// 初始化数据库
let db = remdb::init_global_db(
    &TEST_DB,
    &mut tables,
    &mut primary_indices,
    &mut secondary_indices
).unwrap();

// 进入低功耗模式
db.enter_low_power_mode().unwrap();

// 检查当前低功耗模式状态
let is_low_power = db.is_low_power_mode();

// 在低功耗模式下插入记录
for i in 0..150 {
    match db.get_table_mut(0).unwrap().insert(record_data) {
        Ok(id) => println!("插入成功，记录ID: {}", id),
        Err(e) => println!("插入失败，错误: {:?}", e),
    }
}

// 退出低功耗模式
db.exit_low_power_mode().unwrap();
```

## 平台支持

### POSIX平台

启用POSIX平台支持：

```toml
features = ["posix"]
```

### 裸机平台

启用裸机平台支持：

```toml
features = ["baremetal"]
```

## 测试

### 运行单元测试

```bash
cargo test
```

### 检查编译

在no_std环境下检查编译：

```bash
cargo check --tests --no-default-features
```

在baremetal环境下检查编译：

```bash
cargo check --no-default-features --features=baremetal
```

### 在baremetal环境下运行测试

由于测试框架依赖std库，直接运行`cargo test`在baremetal环境下会失败。但你可以通过以下步骤验证代码在baremetal环境下的正确性：

1. 确保代码可以成功编译：
   ```bash
   cargo check --no-default-features --features=baremetal
   ```

2. 对于实际的baremetal硬件测试，你可能需要：
   - 使用交叉编译工具链
   - 编写针对目标硬件的测试代码
   - 配置适当的链接脚本
   - 使用烧录工具将可执行文件写入硬件

3. 示例交叉编译命令（以ARM Cortex-M为例）：
   ```bash
   cargo build --target thumbv7m-none-eabi --no-default-features --features=baremetal
   ```

## 示例

查看`examples`目录下的示例代码：

- `basic_usage.rs`：基本使用示例，展示表定义、插入、查询和事务操作
- `low_power_mode.rs`：低功耗模式示例，展示如何配置和使用低功耗模式

## 项目结构

```
remdb/
├── src/
│   ├── lib.rs              # 主库入口
│   ├── types.rs            # 基本数据类型定义
│   ├── config.rs           # 编译时配置宏
│   ├── table.rs            # 内存表实现
│   ├── index.rs            # 索引实现
│   ├── transaction.rs      # 事务管理
│   ├── memory/
│   │   ├── allocator.rs    # 静态内存分配器
│   │   ├── pool.rs         # 内存池
│   │   └── mod.rs
│   └── platform/
│       ├── mod.rs          # 平台抽象层定义
│       ├── posix.rs        # POSIX平台实现
│       └── baremetal.rs    # 裸机平台实现
├── examples/
│   ├── basic_usage.rs      # 基本使用示例
│   └── low_power_mode.rs   # 低功耗模式示例
├── tests/
│   ├── unit/
│   │   ├── memory_test.rs  # 内存管理单元测试
│   │   └── table_test.rs   # 表操作单元测试
├── Cargo.toml              # 项目配置
├── Cargo.lock              # 依赖锁定文件
├── PLAN.md                 # 项目计划
└── README.md               # 项目说明文档
```

## 许可证

MIT许可证

## 贡献

欢迎提交问题和拉取请求！

## 注意事项

1. remdb专为嵌入式系统设计，不适合大规模数据存储
2. 在no_std环境下使用时，需要提供适当的内存分配器实现
3. 请确保在使用前正确初始化内存分配器和平台抽象层

## 未来计划

- 支持更多的数据类型
- 优化内存使用
- 提供更多的索引类型
- 增加更多的示例和文档
- 实现更复杂的内存优化算法
- 添加低功耗模式下的性能监控
- 实现自适应低功耗模式，根据系统负载自动切换模式