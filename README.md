# remdb - 嵌入式内存数据库

[English Version](./README_EN.md)

remdb是一个轻量级的嵌入式内存数据库，专为资源受限的嵌入式系统设计，支持no_std环境，具有可预测的内存使用和高性能。

## 主要功能

- **内存表存储**：高效的内存表实现，支持插入、删除、查询和遍历操作
- **索引机制**：
  - 基于哈希的主键索引，提供O(1)的查询性能
  - 多种辅助索引类型：
    - Hash
    - SortedArray
    - BTree（默认）
    - TTree
  - SortedArray、BTree和TTree索引支持范围查询
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
- **增量快照**：
  - 同时支持完整快照和增量快照
  - 增量快照只保存版本号变化的记录
  - 通过仅存储变化数据，减少快照大小和保存时间
  - 支持从增量快照恢复数据
  - 版本管理机制，跟踪数据变化
  - 兼容现有快照格式
- **基于Rust的编译期DDL解析与类型安全代码生成**：
  - 解析SQLite3语法兼容的DDL文件，生成类型安全的Rust代码
  - 支持核心SQLite3 DDL语法：`CREATE TABLE`、`CREATE INDEX`、列定义、`PRIMARY KEY`、`NOT NULL`、`UNIQUE`约束
  - 支持在`CREATE INDEX`语句中指定多种索引类型：`HASH`、`SORTEDARRAY`、`BTREE`（默认）、`TTREE`
  - 编译期执行语法和语义检查，提供清晰错误信息
  - 生成强类型Rust结构体，字段名称、类型与DDL定义严格对应
  - 生成静态表元数据，供数据库运行时使用
  - 生成类型安全的API原型：`insert`、`get_by_id`、`update`、`delete`函数
  - 零运行时开销，使用过程宏实现
- **基于Rust过程宏的零成本DDL集成**：
  - 提供`MemdbTable`过程宏，支持`#[derive(MemdbTable)]`语法
  - 支持内联模式：直接在属性中编写DDL
  - 支持文件模式：关联外部DDL文件
  - 将SQL约束映射为Rust类型系统约束，编译期捕获错误
  - 生成的代码`#[repr(C)]`，内存布局与手写代码完全一致
- **SQL查询支持**：
  - 支持标准SQL SELECT语句查询内存数据库的数据
  - 支持基本查询、条件查询、排序和LIMIT限制
  - 支持比较运算符：`=`、`!=`、`<`、`<=`、`>`、`>=`
  - 提供友好的结果集接口，支持迭代访问和字段获取
- **运行时DDL配置API**：
  - 基于trait的DDL执行器设计，提供`DdlExecutor` trait
  - 支持运行时创建表和索引
  - 支持通过SQL语句执行DDL操作：`CREATE TABLE`、`CREATE INDEX`
  - 支持多种索引类型配置
  - 内存分配器抽象，支持自定义内存分配策略

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

### SQL查询示例

```rust
// 执行SQL查询获取所有用户
let result = db.sql_query("SELECT * FROM users").unwrap();
println!("{}", result.to_string());

// 执行带条件的SQL查询
let result = db.sql_query("SELECT name, age FROM users WHERE age > 25 ORDER BY name ASC LIMIT 10").unwrap();
for row in result {
    println!("{}: {}", row.get(0), row.get(1));
}

// 执行带条件和排序的SQL查询
let result = db.sql_query("SELECT * FROM users WHERE active = true ORDER BY created_at DESC").unwrap();
for row in result {
    println!("ID: {}, Name: {}, Age: {}, Active: {}", 
             row.get(0), row.get(1), row.get(2), row.get(3));
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

### DDL宏使用示例

```rust
use remdb_macros::MemdbTable;

// 使用内联DDL定义带索引的表
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE user (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, active BOOLEAN);
CREATE INDEX idx_user_name ON user USING btree (name);
CREATE INDEX idx_user_age ON user USING hash (age);")]
struct UserTable;

fn main() {
    // 测试生成的User结构体
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        age: Some(30),
        active: Some(true),
    };
    
    println!("生成的User结构体: {:?}", user);
    println!("用户名: {}", user.name);
    println!("年龄: {:?}", user.age);
    
    // 测试数据库配置
    println!("数据库表数量: {}", DATABASE.tables.len());
    
    // 测试API函数（占位符实现）
    // user::insert(&mut db, user);
    // let result = user::get_by_id(&db, 1);
}
```

### 运行时DDL配置API示例

```rust
use remdb::{RemDb, DdlExecutor, types::{DataType, IndexType}};
use remdb::config::{DbConfig, MemoryAllocator};
use core::ptr::NonNull;

// 简单的内存分配器实现
struct SimpleAllocator {
    base_ptr: NonNull<u8>,
    size: usize,
    used: usize,
}

impl SimpleAllocator {
    pub const fn new(base_ptr: NonNull<u8>, size: usize) -> Self {
        Self {
            base_ptr,
            size,
            used: 0,
        }
    }
}

impl MemoryAllocator for SimpleAllocator {
    fn allocate(&self, size: usize) -> Option<NonNull<u8>> {
        let new_used = self.used + size;
        if new_used <= self.size {
            let ptr = NonNull::new((self.base_ptr.as_ptr() as usize + self.used) as *mut u8)?;
            Some(ptr)
        } else {
            None
        }
    }
    
    fn deallocate(&self, _ptr: NonNull<u8>, _size: usize) {
        // 简化实现，不实际释放内存
    }
}

fn main() {
    // 分配内存用于数据库
    let mut buffer = [0u8; 1024 * 1024]; // 1MB
    let base_ptr = NonNull::new(buffer.as_mut_ptr()).unwrap();
    
    // 创建内存分配器
    let allocator = SimpleAllocator::new(base_ptr, buffer.len());
    
    // 创建数据库配置
    let config = DbConfig {
        tables: &[],
        total_memory: buffer.len(),
        low_power_mode_supported: false,
        low_power_max_records: None,
        memory_allocator: &allocator,
    };
    
    // 初始化表和索引数组
    let mut tables = [None; 8];
    let mut primary_indices = [None; 8];
    let mut secondary_indices = [None; 8];
    
    // 创建数据库实例
    let mut db = RemDb::new(
        &config,
        &mut tables,
        &mut primary_indices,
        &mut secondary_indices
    );
    
    // 使用DdlExecutor trait创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32),
            ("name", DataType::String),
            ("age", DataType::UInt8),
            ("active", DataType::Bool),
        ],
        Some(0) // 主键为id字段
    );
    
    // 使用SQL语句创建表
    let result = db.sql_query(
        "CREATE TABLE products (id UINT32 PRIMARY KEY, name STRING, price FLOAT32, in_stock BOOL);"
    );
    
    // 使用DdlExecutor trait创建索引
    let result = db.create_index(
        "users",
        "name",
        IndexType::BTree
    );
    
    // 使用SQL语句创建索引
    let result = db.sql_query(
        "CREATE INDEX idx_product_name ON products (name) USING BTree;"
    );
}
```

#### 文件模式使用示例

```rust
use remdb_macros::MemdbTable;

// 使用外部DDL文件定义带索引的表
#[derive(MemdbTable)]
#[memdb_schema(file = "./schema.ddl")]
struct MyDatabase;

// schema.ddl内容:
// CREATE TABLE user (
//     id INTEGER PRIMARY KEY,
//     name TEXT NOT NULL,
//     email TEXT UNIQUE NOT NULL
// );
//
// CREATE INDEX idx_user_name ON user USING btree (name);
// CREATE INDEX idx_user_email ON user (email); -- 默认使用BTree
//
// CREATE TABLE product (
//     id INTEGER PRIMARY KEY,
//     name TEXT NOT NULL,
//     price REAL NOT NULL,
//     category TEXT
// );
//
// CREATE INDEX idx_product_price ON product USING ttree (price);
// CREATE INDEX idx_product_category ON product USING sortedarray (category);
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
- `incremental_snapshot.rs`：增量快照示例，展示如何保存和恢复增量快照
- `generate_snapshot.rs`：快照生成示例，展示如何生成和使用快照
- `sql_query.rs`：SQL查询示例，展示如何使用SQL查询内存数据库
- `ddl_example.rs`：DDL示例，展示如何使用DDL宏定义表和索引
- `ddl_full_example.rs`：完整DDL示例，展示更复杂的DDL定义
- `ddl_runtime_example.rs`：运行时DDL配置示例，展示如何使用运行时DDL API

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
│   ├── sql/
│   │   ├── mod.rs           # SQL查询模块
│   │   ├── query_parser.rs  # SQL查询解析器
│   │   ├── query_executor.rs # SQL查询执行器
│   │   └── result_set.rs    # 结果集处理
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
│   ├── low_power_mode.rs   # 低功耗模式示例
│   ├── incremental_snapshot.rs # 增量快照示例
│   └── generate_snapshot.rs     # 快照生成示例
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
- 完善运行时DDL配置API，支持完整的表和索引创建功能
- 支持DROP TABLE和ALTER TABLE语句
- 实现更灵活的内存分配策略
- 优化运行时DDL操作的性能
- 支持更复杂的索引配置选项