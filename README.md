# remdb - 嵌入式内存数据库

[English Version](./README_EN.md)

remdb是一个轻量级的嵌入式内存数据库，专为资源受限的嵌入式系统设计，支持no_std环境，具有可预测的内存使用和高性能。

## 主要功能

- **内存表存储**：高效的内存表实现，支持插入、删除、查询和遍历操作
- **索引机制**：
  - 基于哈希的主键索引，提供O(1)的查询性能
  - 多种辅助索引类型：Hash、SortedArray、BTree（默认）、TTree
  - SortedArray、BTree和TTree索引支持范围查询
- **事务支持**：完整的ACID事务支持，包括原子性、一致性、隔离性和持久性
- **内存管理**：支持静态内存分配和动态内存分配，固定大小块内存池
- **平台抽象层**：支持POSIX和裸机环境
- **编译时配置**：使用宏实现表和数据库的编译时配置，优化性能
- **低功耗模式**：优化内存使用，减少事务日志写入频率
- **增量快照**：只保存版本号变化的记录，减少快照大小和保存时间
- **SQL查询支持**：支持标准SQL SELECT语句查询内存数据库的数据，包括聚合函数、数学函数、时间转换函数、JOIN操作和LIKE模式匹配运算符
- **SQL DDL支持**：支持CREATE TABLE和DROP TABLE语句，允许动态创建和删除表结构
- **SQL数据库管理支持**：支持CREATE DATABASE和DROP DATABASE语句，允许创建和管理数据库
- **UTF8字符支持**：完全支持UTF8字符编码，包括字符串存储、字符函数、LIKE运算符和排序
- **数据库监控**：实时监控数据库指标，包括内存使用、查询性能、事务状态等
- **基于UDP的高可靠数据订阅与发布**：支持单播、广播和组播模式，提供基于NACK的重传机制
- **高可用支持**：
  - 主从复制机制，支持一主一从或一主多从拓扑结构
  - 基于心跳机制的自动故障检测和切换
  - 支持同步和异步两种复制一致性模式：
    - 同步模式：主节点等待至少一个从节点确认后才返回，确保数据一致性
    - 异步模式：主节点立即返回，异步复制到从节点，提供更高的性能
  - 自动故障转移，服务中断窗口小于2秒
  - 从节点确认机制：从节点接收到WAL日志后发送确认给主节点
  - 复制状态检查：定期检查复制状态，包括从节点数量、延迟等
  - 支持全量和增量同步：从节点可以请求全量同步或从特定日志索引开始的增量同步
- **向量数据库支持**：
  - 原生向量数据类型：`VECTOR(dimension)`
  - 支持多种距离度量：L2（欧几里得距离）、IP（内积）、COSINE（余弦相似度）
  - 多种向量索引类型：HNSW、HNSW_SQ（带标量量化）、HNSW_BQ（带二值量化）、IVF、IVF_FLAT（带扁平量化）、IVF_PQ（带乘积量化）
  - 向量相似性查询：支持L2距离 `<->`、内积 `<#>`、余弦相似度 `<=>` 操作符
  - 混合查询：支持向量搜索与标量过滤的混合查询
- **时序数据库支持**：
  - 专用的时序表实现，优化时间序列数据存储和查询
  - 支持多种压缩算法
  - 支持时间序列数据分区
  - 支持时间序列数据生命周期管理
  - 支持时间序列数据索引
- **C语言接口**：提供C语言API，方便C/C++应用程序使用

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
# features = ["std", "posix", "pubsub", "ha"]
# 注意：ha依赖pubsub功能，启用ha时会自动启用pubsub
```

### 特性说明

| 特性名 | 依赖 | 描述 |
|-------|------|------|
| std | - | 启用标准库支持 |
| posix | - | 启用POSIX平台支持 |
| pubsub | std | 启用基于UDP的高可靠数据订阅与发布功能 |
| ha | pubsub | 启用高可用支持（主从复制机制） |

## Rust语言的三种使用方式

remdb提供了三种主要的Rust语言使用方式，以满足不同场景的需求：

### 1. 直接定义表数据结构

使用`remdb::table!`宏直接定义表结构，这是最基础的使用方式，适合简单场景：

```rust
#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::alloc::Layout;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

// 直接定义表结构
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
        // 初始化内存分配器
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
        platform::init_platform(platform::posix::get_posix_platform());
        
        // 初始化全局数据库
        let db = init_global_db(
            database!(tables: [users]),
            &mut [None; 1],
            &mut [None; 1],
            &mut [None; 1]
        ).unwrap();
        
        // 使用数据库...
    }
}
```

### 2. 宏定义MemTable

使用`#[derive(MemdbTable)]`宏定义表，支持内联DDL和外部DDL文件，提供更灵活的表定义方式：

#### 内联DDL模式

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
}
```

#### 文件模式

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
```

### 3. 使用DdlExecutor通过动态DDL创建

使用`DdlExecutor` trait在运行时动态创建表和索引，适合需要在运行时灵活配置表结构的场景：

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
        tables: vec![],
        total_memory: buffer.len(),
        low_power_mode_supported: false,
        low_power_max_records: None,
        memory_allocator: &allocator,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_role: remdb::config::HARole::Auto,
        #[cfg(feature = "ha")]
        replication_mode: remdb::config::ReplicationMode::Asynchronous,
        #[cfg(feature = "ha")]
        ha_config: None,
        #[cfg(feature = "ha")]
        replication_sync_timeout: 5000,
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
}
```

## 其他访问方式

### C语言接口访问

remdb提供了C语言接口，方便C/C++应用程序使用：

```c
#include "remdb_c.h"

int main() {
    // 初始化数据库
    remdb_t *db = remdb_init();
    
    // 创建表
    remdb_create_table(db, "users", ...);
    
    // 插入数据
    remdb_insert(db, "users", ...);
    
    // 查询数据
    remdb_result_t *result = remdb_query(db, "SELECT * FROM users");
    
    // 处理结果...
    
    // 释放资源
    remdb_free_result(result);
    remdb_close(db);
    
    return 0;
}
```

### JDBC访问

remdb提供了JDBC驱动，允许Java应用程序通过JDBC API访问remdb数据库：

```java
import java.sql.*;

public class RemdbExample {
    public static void main(String[] args) {
        try {
            // 加载驱动
            Class.forName("com.remdb.jdbc.Driver");
            
            // 建立连接
            String url = "jdbc:remdb://localhost:8080/dbname";
            Connection conn = DriverManager.getConnection(url);
            
            // 创建Statement
            Statement stmt = conn.createStatement();
            
            // 执行查询
            ResultSet rs = stmt.executeQuery("SELECT * FROM users");
            
            // 处理结果集
            while (rs.next()) {
                System.out.println(rs.getInt("id") + ": " + rs.getString("name"));
            }
            
            // 关闭资源
            rs.close();
            stmt.close();
            conn.close();
            
        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
```

### 基于UDP的高可靠数据订阅与发布

> 注意：使用此功能需要在Cargo.toml中启用`pubsub`特性

remdb提供了基于UDP的高可靠数据订阅与发布机制，支持单播、广播和组播模式，适合分布式系统中的数据同步。系统内置了多种预定义主题，用于发布不同类型的数据库事件：

#### 预定义主题

| 主题名称 | 描述 | 消息格式 |
|---------|------|---------|
| wal | 所有WAL操作 | WAL_LOG_<id>: Operation=<operation_type>, Table=<table_name>, ID=<record_id>, Data=<data> |
| tables | 表创建/删除事件 | CREATE:table=<table_name>,id=<table_id>,fields=<field_count> 或 DELETE:table=<table_name>,id=<table_id> |
| metrics | 数据库指标 | JSON格式的数据库指标数据 |
| healthstatus | 健康状态 | JSON格式的健康状态数据 |
| table.<table_name> | 表内容变更 | INSERT:table=<table_name>,id=<record_id>,data=<hex_data> 或 UPDATE:table=<table_name>,id=<record_id>,data=<hex_data> |

#### 使用示例

```rust
use std::time::Duration;
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};

// 创建发布/订阅配置
let config = PubSubConfig {
    udp_mode: UdpMode::Broadcast,
    multicast_addr: None,
    port: 5555,
    max_topics: 32,
    max_subscribers_per_topic: 16,
    buffer_size: 4096,
    enable_nack: true,
    retransmit_timeout: Duration::from_millis(100),
    max_retransmits: 3,
    heartbeat_interval: Duration::from_secs(10),
    frame_pool_size: 128,
};

// 创建发布/订阅实例
let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
pubsub.init().expect("Failed to initialize PubSub");

// 定义订阅回调
let callback = |topic_id: u16, data: &[u8]| -> bool {
    println!("Received data on topic {}: {:?}", topic_id, String::from_utf8_lossy(data));
    true
};

// 订阅主题
let subscription_id = pubsub.subscribe(0, callback).expect("Failed to subscribe");

// 发布数据
let msg = "Hello, PubSub!";
pubsub.publish(0, msg.as_bytes()).expect("Failed to publish");

// 取消订阅
pubsub.unsubscribe(subscription_id).expect("Failed to unsubscribe");
```

## SQL查询示例

remdb支持标准SQL SELECT语句查询内存数据库的数据，包括各种聚合函数、数学函数和时间转换函数：

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

// 使用时间转换函数
let result = db.sql_query("SELECT id, name, TO_ISO8601(created_at) AS iso_created FROM users").unwrap();

// 使用TO_CHAR函数格式化时间
let result = db.sql_query("SELECT id, name, TO_CHAR(created_at, 'YYYY-MM-DD HH24:MI:SS') AS formatted_date FROM users").unwrap();

// 使用TO_EPOCH函数获取Unix时间戳
let result = db.sql_query("SELECT id, name, TO_EPOCH(created_at) AS unix_time FROM users").unwrap();

// 结合聚合函数和时间函数
let result = db.sql_query("SELECT TO_CHAR(timestamp, 'YYYY-MM-DD') AS date, AVG(value) AS avg_value FROM sensor_data GROUP BY date").unwrap();

// 使用LIKE运算符进行模式匹配
let result = db.sql_query("SELECT * FROM users WHERE name LIKE 'A%'").unwrap(); // 匹配以'A'开头的名称
let result = db.sql_query("SELECT * FROM users WHERE name LIKE '%son'").unwrap(); // 匹配以'son'结尾的名称
let result = db.sql_query("SELECT * FROM users WHERE name LIKE '%mi%'").unwrap(); // 匹配包含'mi'的名称
let result = db.sql_query("SELECT * FROM users WHERE name LIKE 'A__e'").unwrap(); // 匹配长度为4且以'A'开头、以'e'结尾的名称
```

## 时序数据库

remdb提供了强大的时序数据库功能，专为时间序列数据的高效存储和查询而设计：

### 基本使用

```rust
use remdb::*;
use remdb::time_series::*;
use std::time::{Duration, SystemTime};

// 定义时序表结构
remdb::table!(
    sensor_data,
    5000, // 最大记录数
    primary_key: id,
    secondary_index: timestamp,
    fields: {
        id: i32,
        sensor_id: str(32),  // 传感器ID
        sensor_type: str(32), // 传感器类型
        value: f64,           // 传感器数值
        timestamp: u64,       // 时间戳
        location: str(64)     // 位置信息
    }
);

// 定义数据库配置
remdb::database!(
    DB_CONFIG,
    tables: [sensor_data]
);

fn main() {
    unsafe {
        // 初始化内存分配器
        let memory_size = 128 * 1024 * 1024; // 128MB
        static mut DB_MEMORY: [u8; 128 * 1024 * 1024] = [0u8; 128 * 1024 * 1024];
        
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        ).expect("Failed to initialize memory allocator");
        
        // 初始化平台抽象层
        platform::init_platform(platform::posix::get_posix_platform());
        
        // 初始化全局数据库
        let db = init_global_db(&DB_CONFIG).unwrap();
        
        // 获取表引用
        let table_mut = db.get_table_mut(0).unwrap();
        
        // 模拟插入传感器数据...
        
        // 查询时间范围内的数据
        let start_time = base_time;
        let end_time = base_time + 30 * 60000; // 30分钟
        
        let mut result_buffer = [0u8; 160 * 50]; // 50条记录的缓冲区
        let found_count = table_mut.get_records_in_time_window(
            4, // timestamp字段索引
            start_time,
            end_time,
            result_buffer.as_mut_ptr(),
            50
        ).unwrap();
        
        // 计算时间范围内的统计信息
        match table_mut.aggregate_count(4, start_time, end_time) {
            Ok(count) => {
                println!("时间范围内记录数: {}", count);
                // 计算平均值、总和、最小值、最大值...
            },
            Err(e) => println!("统计记录数失败: {:?}", e)
        }
    }
}
```

## 向量数据库

remdb提供了强大的向量数据库功能，支持原生向量类型、多种距离度量和高效的向量索引：

### 基本使用

```rust
use remdb::*;
use remdb::config::{DbConfig, WALConfig};

// 初始化数据库配置
let config = Box::leak(Box::new(DbConfig {
    tables: vec![],
    total_memory: 16 * 1024 * 1024, // 16MB
    low_power_mode_supported: false,
    low_power_max_records: None,
    memory_allocator: &SimpleAllocator,
    wal_config: WALConfig {
        log_path: "./wal".to_string(),
        log_mode: remdb::config::LogMode::Async,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 4 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 2,
    },
    time_series_defaults: TimeSeriesConfig {
        partition_duration_secs: 3600,
        retention_period_secs: 7 * 24 * 3600,
        compression: remdb::time_series::compression::CompressionType::None,
        max_partitions: 100,
    },
}));

// 初始化数据库
let mut db = RemDb::new(config);
db.init()?;

// 创建包含向量字段的表
let create_sql = r#"CREATE TABLE products (
    id INT32 PRIMARY KEY,
    name TEXT,
    embedding VECTOR(4) WITH DISTANCE=IP
)"#;
db.sql_query(create_sql)?;

// 插入向量数据
let insert_sql = r#"INSERT INTO products (id, name, embedding) VALUES
    (1, 'product1', '[0.1, 0.2, 0.3, 0.4]'),
    (2, 'product2', '[1.0, 0.9, 0.8, 0.7]')
"#;
db.sql_query(insert_sql)?;

// 向量相似性查询 - 内积距离
let similarity_sql = "SELECT id, name, embedding <#> '[0.2, 0.3, 0.4, 0.5]' AS similarity FROM products ORDER BY similarity DESC LIMIT 2";
let similarity_result = db.sql_query(similarity_sql)?;

// 创建向量索引
let create_index_sql = "CREATE INDEX idx_products_embedding ON products (embedding) USING HNSW WITH (M=16, ef_construction=200)";
db.sql_query(create_index_sql)?;
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

### 运行核心库测试

```bash
cargo test --lib
```

### 运行带有特定特性的核心库测试

```bash
cargo test --lib --features "pubsub ha"
```

### 运行完整测试套件

```bash
cargo test
```

### 检查编译

在no_std环境下检查编译：

```bash
cargo check --tests --no-default-features
```

### 在baremetal环境下检查编译：

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

### 测试注意事项

- 核心库测试（`cargo test --lib`）不依赖于特定特性，是验证基本功能的最佳方式
- 完整测试套件（`cargo test`）可能会因为示例和集成测试依赖特定特性而失败
- 带有特性的测试（如`--features "pubsub ha"`）需要确保相关特性已正确配置
- 部分示例和集成测试可能需要特定的运行环境或配置

## 示例

查看`examples`目录下的示例代码：

- `basic_usage.rs`：基本使用示例，展示表定义、插入、查询和事务操作
- `low_power_mode.rs`：低功耗模式示例，展示如何配置和使用低功耗模式
- `incremental_snapshot.rs`：增量快照示例，展示如何保存和恢复增量快照
- `sql_query.rs`：SQL查询示例，展示如何使用SQL查询内存数据库
- `ddl_example.rs`：DDL示例，展示如何使用DDL宏定义表和索引
- `ddl_runtime_example.rs`：运行时DDL配置示例，展示如何使用运行时DDL API
- `pubsub_example.rs`：发布/订阅示例，展示如何使用基于UDP的高可靠数据订阅与发布功能
- `time_series.rs`：时间序列示例，展示如何处理时间序列数据
- `vector_example.rs`：向量数据库示例，展示如何使用向量字段、插入向量数据和执行向量相似性查询
- `vector_distance_test.rs`：向量距离测试示例，展示不同距离度量的向量相似性计算
- `drop_table_example.rs`：DROP TABLE示例，展示如何使用SQL DROP TABLE语句删除表结构
- `test_remdb_server.rs`：主从复制示例，展示如何使用同步或异步复制模式运行主从服务器

### 主从复制示例

> 注意：使用此功能需要在Cargo.toml中启用`ha`特性

`test_remdb_server.rs`示例展示了如何使用主从复制功能，支持通过命令行参数设置同步或异步复制模式：

#### 主节点启动命令

```bash
# 同步模式
cargo run --example test_remdb_server master sync

# 异步模式
cargo run --example test_remdb_server master async
```

#### 从节点启动命令

```bash
# 同步模式
cargo run --example test_remdb_server slave sync <master_ip> <master_port>

# 异步模式
cargo run --example test_remdb_server slave async <master_ip> <master_port>
```

#### 示例输出

```
Starting RemDB Server...
Role: Master
Replication Mode: Sync
RemDB Server started successfully!
Listening on UDP port 5555
Topics available:
- WAL_INSERT (ID: 1) - WAL insert operations
- WAL_UPDATE (ID: 2) - WAL update operations
- WAL_DELETE (ID: 3) - WAL delete operations
- WAL_TIMESERIES_INSERT (ID: 4) - WAL timeseries insert operations
- WAL_COMMIT (ID: 5) - WAL commit operations
- WAL_ABORT (ID: 6) - WAL abort operations
- WAL_CHECKPOINT (ID: 7) - WAL checkpoint operations
- WAL_ALL (ID: 8) - All WAL operations
- TABLES (ID: 9) - Table creation/deletion events
- HEARTBEAT - Sent every 5 seconds
```

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
│   ├── monitor.rs          # 数据库监控模块
│   ├── c_api.rs            # C语言接口实现
│   ├── sql/
│   │   ├── mod.rs           # SQL查询模块
│   │   ├── query_parser.rs  # SQL查询解析器
│   │   ├── query_executor.rs # SQL查询执行器
│   │   └── result_set.rs    # 结果集处理
│   ├── memory/
│   │   ├── allocator.rs    # 静态内存分配器
│   │   ├── pool.rs         # 内存池
│   │   └── mod.rs
│   ├── platform/
│   │   ├── mod.rs          # 平台抽象层定义
│   │   ├── posix.rs        # POSIX平台实现
│   │   └── baremetal.rs    # 裸机平台实现
│   ├── ha/
│   │   ├── mod.rs          # 高可用模块入口
│   │   ├── manager.rs      # HA管理器实现
│   │   ├── replication.rs  # 复制功能实现
│   │   ├── heartbeat.rs    # 心跳检测实现
│   │   └── role.rs         # 角色管理实现
│   ├── pubsub/
│   │   ├── mod.rs          # 发布/订阅模块入口
│   │   ├── protocol.rs     # 协议帧定义与解析
│   │   ├── udp.rs          # 跨平台UDP套接字封装
│   │   ├── subscriber.rs   # 订阅者管理
│   │   ├── publisher.rs    # 发布者管理
│   │   ├── topics.rs       # 预定义主题
│   │   ├── ttl_ringbuffer.rs # TTL环形缓冲区
│   │   └── crc32.rs        # CRC32校验实现
│   └── time_series/
│       ├── mod.rs          # 时序数据库模块入口
│       ├── table.rs        # 时序表实现
│       ├── index.rs        # 时序数据索引
│       ├── compression.rs  # 压缩算法实现
│       ├── partition.rs    # 数据分区实现
│       ├── lifecycle.rs    # 数据生命周期管理
│       └── config.rs       # 时序数据库配置
├── examples/               # 示例代码
├── tests/                  # 测试代码
├── Cargo.toml              # 项目配置
└── README.md               # 项目说明文档
```

## 许可证

MIT许可证

## 贡献

欢迎提交问题和拉取请求！

## 项目链接
- 国内：https://gitee.com/totaltrust/remdb
- 国外：https://github.com/bobjia/remdb
- Crates：https://crates.io/crates/remdb

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
- 完善运行时DDL配置API，支持完整的表和索引创建功能
- 支持ALTER TABLE语句
- 优化运行时DDL操作的性能
- 支持更复杂的索引配置选项