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
- **SQL查询支持**：支持标准SQL SELECT语句查询内存数据库的数据
- **基于UDP的高可靠数据订阅与发布**：支持单播、广播和组播模式，提供基于NACK的重传机制
- **高可用支持**：
  - 主从复制机制，支持一主一从或一主多从拓扑结构
  - 基于心跳机制的自动故障检测和切换
  - 支持同步和异步两种复制一致性模式
  - 自动故障转移，服务中断窗口小于2秒
- **时序数据库支持**：专用的时序表实现，优化时间序列数据存储和查询

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

remdb提供了基于UDP的高可靠数据订阅与发布机制，支持单播、广播和组播模式，适合分布式系统中的数据同步。系统内置了多种预定义主题，用于发布不同类型的数据库事件：

#### 预定义主题

| 主题名称 | 描述 | 消息格式 |
|---------|------|---------|
| wal.insert | WAL插入操作 | WAL_LOG_<id>: Operation=INSERT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.update | WAL更新操作 | WAL_LOG_<id>: Operation=UPDATE, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.delete | WAL删除操作 | WAL_LOG_<id>: Operation=DELETE, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.timeseriesInsert | WAL时序插入操作 | WAL_LOG_<id>: Operation=TIMESERIES_INSERT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.commit | WAL提交操作 | WAL_LOG_<id>: Operation=COMMIT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.abort | WAL回滚操作 | WAL_LOG_<id>: Operation=ABORT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.checkpoint | WAL检查点操作 | WAL_LOG_<id>: Operation=CHECKPOINT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.* | 所有WAL操作（通配符） | 与具体WAL操作相同 |
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

remdb支持标准SQL SELECT语句查询内存数据库的数据：

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
│   └── pubsub/
│       ├── mod.rs          # 发布/订阅模块入口
│       ├── protocol.rs     # 协议帧定义与解析
│       ├── udp.rs          # 跨平台UDP套接字封装
│       ├── subscriber.rs   # 订阅者管理
│       ├── publisher.rs    # 发布者管理
│       └── crc32.rs       # CRC32校验实现
├── examples/               # 示例代码
├── tests/                  # 测试代码
├── Cargo.toml              # 项目配置
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
- 完善运行时DDL配置API，支持完整的表和索引创建功能
- 支持DROP TABLE和ALTER TABLE语句
- 优化运行时DDL操作的性能
- 支持更复杂的索引配置选项