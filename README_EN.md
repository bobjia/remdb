# remdb - Embedded In-Memory Database

[中文版](./README.md)

remdb is a lightweight embedded in-memory database designed for resource-constrained embedded systems, supporting no_std environments with predictable memory usage and high performance.

## Key Features

- **In-Memory Table Storage**: Efficient in-memory table implementation supporting insert, delete, query, and traversal operations
- **Indexing Mechanisms**: 
  - Hash-based primary key index providing O(1) query performance
  - Multiple secondary index types: Hash, SortedArray, BTree (default), TTree
  - Support for range queries with SortedArray, BTree and TTree indices
- **Transaction Support**: Complete ACID transaction support, including atomicity, consistency, isolation, and durability
- **Memory Management**: Supports static and dynamic memory allocation with fixed-size block memory pool
- **Platform Abstraction Layer**: Supports both POSIX and baremetal environments
- **Compile-time Configuration**: Table and database configuration via macros for performance optimization
- **Low Power Mode**: Optimized memory usage with reduced transaction log write frequency
- **Incremental Snapshot**: Only saves records with changed version numbers, reducing snapshot size and save time
- **SQL Query Support**: Supports standard SQL SELECT statements to query in-memory database data
- **UDP-based Reliable Data Pub/Sub**: Supports unicast, broadcast, and multicast modes with NACK-based retransmission
- **High Availability Support**:
  - Master-slave replication mechanism supporting one-master-one-slave or one-master-multi-slave topology
  - Automatic failure detection and failover based on heartbeat mechanism
  - Support for both synchronous and asynchronous replication consistency modes
  - Automatic failover with service interruption window less than 2 seconds
- **Time Series Database Support**: Dedicated time series table implementation optimized for time series data storage and querying

## Technical Characteristics

- **Zero External Dependencies**: No external library dependencies, supports no_std environments
- **Predictable Memory Usage**: Static memory allocation suitable for resource-constrained embedded systems
- **Compile-time Optimization**: Compile-time configuration via macros reduces runtime overhead
- **Multi-platform Support**: Supports both POSIX and baremetal environments
- **Type Safety**: Leverages Rust's type system to ensure data safety
- **Efficient Synchronization**: Implements spinlock synchronization mechanism suitable for multi-threaded environments

## Quick Start

### Installation

Add remdb to your Cargo.toml file:

```toml
[dependencies]
remdb = { path = "./remdb", default-features = false }

# Optional features
# features = ["std", "posix"]
```

## Three Ways to Use remdb with Rust

remdb provides three main ways to use it with Rust to meet different scenario requirements:

### 1. Direct Table Data Structure Definition

Use the `remdb::table!` macro to directly define table structures, which is the most basic usage suitable for simple scenarios:

```rust
#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::alloc::Layout;
use remdb::*;

// Define memory buffer
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

// Directly define table structure
remdb::table!(
    users,
    100, // Maximum record count
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(32), // 32-byte fixed-length string
        age: i8,
        active: bool,
        created_at: u64
    }
);

// Define database configuration
remdb::database!(
    tables: [users]
);

// Memory allocation error handler
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Allocation error: {:?}", layout);
}

fn main() {
    unsafe {
        // Initialize memory allocator
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // Initialize platform abstraction layer
        platform::init_platform(platform::posix::get_posix_platform());
        
        // Initialize global database
        let db = init_global_db(
            database!(tables: [users]),
            &mut [None; 1],
            &mut [None; 1],
            &mut [None; 1]
        ).unwrap();
        
        // Use database...
    }
}
```

### 2. MemTable Definition with Macros

Use the `#[derive(MemdbTable)]` macro to define tables, supporting inline DDL and external DDL files for more flexible table definition:

#### Inline DDL Mode

```rust
use remdb_macros::MemdbTable;

// Define table with indexes using inline DDL
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE user (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, active BOOLEAN);
CREATE INDEX idx_user_name ON user USING btree (name);
CREATE INDEX idx_user_age ON user USING hash (age);")]
struct UserTable;

fn main() {
    // Test generated User struct
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        age: Some(30),
        active: Some(true),
    };
    
    println!("Generated User struct: {:?}", user);
    println!("User name: {}", user.name);
    println!("User age: {:?}", user.age);
}
```

#### File Mode

```rust
use remdb_macros::MemdbTable;

// Define tables with indexes using external DDL file
#[derive(MemdbTable)]
#[memdb_schema(file = "./schema.ddl")]
struct MyDatabase;

// schema.ddl content:
// CREATE TABLE user (
//     id INTEGER PRIMARY KEY,
//     name TEXT NOT NULL,
//     email TEXT UNIQUE NOT NULL
// );
//
// CREATE INDEX idx_user_name ON user USING btree (name);
// CREATE INDEX idx_user_email ON user (email); -- Default to BTree
```

### 3. Dynamic DDL Creation with DdlExecutor

Use the `DdlExecutor` trait to dynamically create tables and indexes at runtime, suitable for scenarios requiring flexible configuration:

```rust
use remdb::{RemDb, DdlExecutor, types::{DataType, IndexType}};
use remdb::config::{DbConfig, MemoryAllocator};
use core::ptr::NonNull;

// Simple memory allocator implementation
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
        // Simplified implementation, no actual memory deallocation
    }
}

fn main() {
    // Allocate memory for database
    let mut buffer = [0u8; 1024 * 1024]; // 1MB
    let base_ptr = NonNull::new(buffer.as_mut_ptr()).unwrap();
    
    // Create memory allocator
    let allocator = SimpleAllocator::new(base_ptr, buffer.len());
    
    // Create database configuration
    let config = DbConfig {
        tables: &[],
        total_memory: buffer.len(),
        low_power_mode_supported: false,
        low_power_max_records: None,
        memory_allocator: &allocator,
    };
    
    // Initialize table and index arrays
    let mut tables = [None; 8];
    let mut primary_indices = [None; 8];
    let mut secondary_indices = [None; 8];
    
    // Create database instance
    let mut db = RemDb::new(
        &config,
        &mut tables,
        &mut primary_indices,
        &mut secondary_indices
    );
    
    // Create table using DdlExecutor trait
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32),
            ("name", DataType::String),
            ("age", DataType::UInt8),
            ("active", DataType::Bool),
        ],
        Some(0) // Primary key is id field
    );
    
    // Create table using SQL statement
    let result = db.sql_query(
        "CREATE TABLE products (id UINT32 PRIMARY KEY, name STRING, price FLOAT32, in_stock BOOL);"
    );
    
    // Create index using DdlExecutor trait
    let result = db.create_index(
        "users",
        "name",
        IndexType::BTree
    );
}
```

## Other Access Methods

### C Language Interface Access

remdb provides a C language interface for C/C++ applications:

```c
#include "remdb_c.h"

int main() {
    // Initialize database
    remdb_t *db = remdb_init();
    
    // Create table
    remdb_create_table(db, "users", ...);
    
    // Insert data
    remdb_insert(db, "users", ...);
    
    // Query data
    remdb_result_t *result = remdb_query(db, "SELECT * FROM users");
    
    // Process results...
    
    // Free resources
    remdb_free_result(result);
    remdb_close(db);
    
    return 0;
}
```

### JDBC Access

remdb provides a JDBC driver, allowing Java applications to access remdb databases through JDBC API:

```java
import java.sql.*;

public class RemdbExample {
    public static void main(String[] args) {
        try {
            // Load driver
            Class.forName("com.remdb.jdbc.Driver");
            
            // Establish connection
            String url = "jdbc:remdb://localhost:8080/dbname";
            Connection conn = DriverManager.getConnection(url);
            
            // Create Statement
            Statement stmt = conn.createStatement();
            
            // Execute query
            ResultSet rs = stmt.executeQuery("SELECT * FROM users");
            
            // Process result set
            while (rs.next()) {
                System.out.println(rs.getInt("id") + ": " + rs.getString("name"));
            }
            
            // Close resources
            rs.close();
            stmt.close();
            conn.close();
            
        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
```

### UDP-based Reliable Data Subscription and Publishing

remdb provides a UDP-based reliable data publish/subscribe mechanism, supporting unicast, broadcast, and multicast modes, suitable for data synchronization in distributed systems. The system includes several predefined topics for publishing different types of database events:

#### Predefined Topics

| Topic Name | Description | Message Format |
|-----------|-------------|---------------|
| wal.insert | WAL insert operation | WAL_LOG_<id>: Operation=INSERT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.update | WAL update operation | WAL_LOG_<id>: Operation=UPDATE, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.delete | WAL delete operation | WAL_LOG_<id>: Operation=DELETE, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.timeseriesInsert | WAL timeseries insert operation | WAL_LOG_<id>: Operation=TIMESERIES_INSERT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.commit | WAL commit operation | WAL_LOG_<id>: Operation=COMMIT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.abort | WAL abort operation | WAL_LOG_<id>: Operation=ABORT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.checkpoint | WAL checkpoint operation | WAL_LOG_<id>: Operation=CHECKPOINT, Table=<table_name>, ID=<record_id>, Data=<data> |
| wal.* | All WAL operations (wildcard) | Same as specific WAL operation |
| tables | Table creation/deletion events | CREATE:table=<table_name>,id=<table_id>,fields=<field_count> or DELETE:table=<table_name>,id=<table_id> |
| metrics | Database metrics | JSON-formatted database metrics data |
| healthstatus | Health status | JSON-formatted health status data |
| table.<table_name> | Table content changes | INSERT:table=<table_name>,id=<record_id>,data=<hex_data> or UPDATE:table=<table_name>,id=<record_id>,data=<hex_data> |

#### Usage Example

```rust
use std::time::Duration;
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};

// Create publish/subscribe configuration
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

// Create publish/subscribe instance
let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
pubsub.init().expect("Failed to initialize PubSub");

// Define subscription callback
let callback = |topic_id: u16, data: &[u8]| -> bool {
    println!("Received data on topic {}: {:?}", topic_id, String::from_utf8_lossy(data));
    true
};

// Subscribe to topic
let subscription_id = pubsub.subscribe(0, callback).expect("Failed to subscribe");

// Publish data
let msg = "Hello, PubSub!";
pubsub.publish(0, msg.as_bytes()).expect("Failed to publish");

// Unsubscribe
pubsub.unsubscribe(subscription_id).expect("Failed to unsubscribe");
```

## SQL Query Examples

remdb supports standard SQL SELECT statements to query data in the in-memory database:

```rust
// Execute SQL query to get all users
let result = db.sql_query("SELECT * FROM users").unwrap();
println!("{}", result.to_string());

// Execute SQL query with condition
let result = db.sql_query("SELECT name, age FROM users WHERE age > 25 ORDER BY name ASC LIMIT 10").unwrap();
for row in result {
    println!("{}: {}", row.get(0), row.get(1));
}

// Execute SQL query with condition and sorting
let result = db.sql_query("SELECT * FROM users WHERE active = true ORDER BY created_at DESC").unwrap();
for row in result {
    println!("ID: {}, Name: {}, Age: {}, Active: {}", 
             row.get(0), row.get(1), row.get(2), row.get(3));
}
```

## Time Series Database

remdb provides powerful time series database functionality, specifically designed for efficient storage and querying of time series data:

### Basic Usage

```rust
use remdb::*;
use remdb::time_series::*;
use std::time::{Duration, SystemTime};

// Define time series table structure
remdb::table!(
    sensor_data,
    5000, // Maximum record count
    primary_key: id,
    secondary_index: timestamp,
    fields: {
        id: i32,
        sensor_id: str(32),  // Sensor ID
        sensor_type: str(32), // Sensor type
        value: f64,           // Sensor value
        timestamp: u64,       // Timestamp
        location: str(64)     // Location information
    }
);

// Define database configuration
remdb::database!(
    DB_CONFIG,
    tables: [sensor_data]
);

fn main() {
    unsafe {
        // Initialize memory allocator
        let memory_size = 128 * 1024 * 1024; // 128MB
        static mut DB_MEMORY: [u8; 128 * 1024 * 1024] = [0u8; 128 * 1024 * 1024];
        
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        ).expect("Failed to initialize memory allocator");
        
        // Initialize platform abstraction layer
        platform::init_platform(platform::posix::get_posix_platform());
        
        // Initialize global database
        let db = init_global_db(&DB_CONFIG).unwrap();
        
        // Get table reference
        let table_mut = db.get_table_mut(0).unwrap();
        
        // Simulate inserting sensor data...
        
        // Query data within time range
        let start_time = base_time;
        let end_time = base_time + 30 * 60000; // 30 minutes
        
        let mut result_buffer = [0u8; 160 * 50]; // Buffer for 50 records
        let found_count = table_mut.get_records_in_time_window(
            4, // timestamp field index
            start_time,
            end_time,
            result_buffer.as_mut_ptr(),
            50
        ).unwrap();
        
        // Calculate statistics within time range
        match table_mut.aggregate_count(4, start_time, end_time) {
            Ok(count) => {
                println!("Record count within time range: {}", count);
                // Calculate average, sum, min, max...
            },
            Err(e) => println!("Failed to count records: {:?}", e)
        }
    }
}
```

## Platform Support

### POSIX Platform

Enable POSIX platform support:

```toml
features = ["posix"]
```

### Baremetal Platform

Enable baremetal platform support:

```toml
features = ["baremetal"]
```

## Testing

### Run Unit Tests

```bash
cargo test
```

### Check Compilation

Check compilation in no_std environment:

```bash
cargo check --tests --no-default-features
```

### Check Compilation in baremetal environment:

```bash
cargo check --no-default-features --features=baremetal
```

### Running Tests in Baremetal Environment

Due to the test framework's dependency on the std library, directly running `cargo test` in a baremetal environment will fail. However, you can verify the correctness of the code in a baremetal environment through the following steps:

1. Ensure the code compiles successfully:
   ```bash
   cargo check --no-default-features --features=baremetal
   ```

2. For actual baremetal hardware testing, you may need:
   - Cross-compilation toolchain
   - Test code written for the target hardware
   - Appropriate linker script configuration
   - Flashing tool to write the executable to hardware

3. Example cross-compilation command (for ARM Cortex-M):
   ```bash
   cargo build --target thumbv7m-none-eabi --no-default-features --features=baremetal
   ```

## Examples

Check the examples directory for sample code:

- `basic_usage.rs`: Basic usage example demonstrating table definition, insertion, query, and transaction operations
- `low_power_mode.rs`: Low power mode example demonstrating how to configure and use low power mode
- `incremental_snapshot.rs`: Incremental snapshot example demonstrating how to save and restore incremental snapshots
- `sql_query.rs`: SQL query example demonstrating how to use SQL to query the in-memory database
- `ddl_example.rs`: DDL example demonstrating how to define tables and indexes using DDL macros
- `ddl_runtime_example.rs`: Runtime DDL configuration example demonstrating how to use the runtime DDL API
- `pubsub_example.rs`: Pub/Sub example demonstrating how to use the UDP-based reliable data publish/subscribe functionality
- `time_series.rs`: Time series example demonstrating how to handle time series data

## Project Structure

```
remdb/
├── src/
│   ├── lib.rs              # Main library entry point
│   ├── types.rs            # Basic data type definitions
│   ├── config.rs           # Compile-time configuration macros
│   ├── table.rs            # In-memory table implementation
│   ├── index.rs            # Index implementation
│   ├── transaction.rs      # Transaction management
│   ├── sql/
│   │   ├── mod.rs           # SQL query module
│   │   ├── query_parser.rs  # SQL query parser
│   │   ├── query_executor.rs # SQL query executor
│   │   └── result_set.rs    # Result set handling
│   ├── memory/
│   │   ├── allocator.rs    # Static memory allocator
│   │   ├── pool.rs         # Memory pool
│   │   └── mod.rs
│   ├── platform/
│   │   ├── mod.rs          # Platform abstraction layer definition
│   │   ├── posix.rs        # POSIX platform implementation
│   │   └── baremetal.rs    # Baremetal platform implementation
│   ├── ha/
│   │   ├── mod.rs          # High Availability module entry
│   │   ├── manager.rs      # HA Manager implementation
│   │   ├── replication.rs  # Replication functionality implementation
│   │   ├── heartbeat.rs    # Heartbeat monitoring implementation
│   │   └── role.rs         # Role management implementation
│   └── pubsub/
│       ├── mod.rs          # Pub/Sub module entry
│       ├── protocol.rs     # Protocol frame definition and parsing
│       ├── udp.rs          # Cross-platform UDP socket encapsulation
│       ├── subscriber.rs   # Subscriber management
│       ├── publisher.rs    # Publisher management
│       └── crc32.rs       # CRC32 check implementation
├── examples/               # Example code
├── tests/                  # Test code
├── Cargo.toml              # Project configuration
└── README.md               # Project documentation
```

## License

MIT License

## Contribution

Issues and pull requests are welcome!

## Notes

1. remdb is designed for embedded systems and is not suitable for large-scale data storage
2. When used in no_std environments, appropriate memory allocator implementation needs to be provided
3. Ensure proper initialization of memory allocator and platform abstraction layer before use

## Future Plans

- Support more data types
- Optimize memory usage
- Provide more index types
- Add more examples and documentation
- Implement more complex memory optimization algorithms
- Complete runtime DDL configuration API, supporting full table and index creation functionality
- Support DROP TABLE and ALTER TABLE statements
- Implement more flexible memory allocation strategies
- Optimize performance of runtime DDL operations
- Support more complex index configuration options