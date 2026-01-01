# remdb - Embedded In-Memory Database

[中文版](./README.md)

remdb is a lightweight embedded in-memory database designed for resource-constrained embedded systems, supporting no_std environments with predictable memory usage and high performance.

## Key Features

- **In-Memory Table Storage**: Efficient in-memory table implementation supporting insert, delete, query, and traversal operations
- **Indexing Mechanisms**:
  - Hash-based primary key index providing O(1) query performance
  - Multiple secondary index types:
    - Hash
    - SortedArray
    - BTree (default)
    - TTree
  - Support for range queries with SortedArray, BTree and TTree indices
- **Transaction Support**: Complete ACID transaction support, including:
  - Atomicity: Transactions are either fully committed or fully rolled back
  - Consistency: Ensures data integrity and correctness
  - Isolation: Supports multiple isolation levels (Read Uncommitted, Read Committed, Repeatable Read, Serializable)
  - Durability: Ensures data persistence through Write-Ahead Logging (WAL)
  - Supports record-level locking (shared locks and exclusive locks)
  - Supports transaction logging and crash recovery
- **Memory Management**:
  - Supports static memory allocation and dynamic memory allocation
  - Fixed-size block memory pool enabling efficient memory management
  - Dynamic memory allocator, supporting runtime DDL operations
- **Platform Abstraction Layer**: Supports both POSIX and baremetal environments
- **Compile-time Configuration**: Table and database configuration implemented via macros for performance optimization
- **Low Power Mode**:
  - Supports entering and exiting low power mode
  - Optimized memory usage in low power mode
  - Reduced transaction log write frequency, lowering disk I/O
  - Automatically overwrites oldest records when record count exceeds limits
- **Incremental Snapshot**:
  - Supports both full snapshot and incremental snapshot
  - Incremental snapshot only saves records with changed version numbers
  - Reduces snapshot size and save time by only storing changed data
  - Supports restoring data from incremental snapshots
  - Version management mechanism to track data changes
  - Compatible with existing snapshot format
- **Rust-based Compile-time DDL Parsing and Type-safe Code Generation**:
  - Parses SQLite3 syntax-compatible DDL files and generates type-safe Rust code
  - Supports core SQLite3 DDL syntax: `CREATE TABLE`, `CREATE INDEX`, column definitions, `PRIMARY KEY`, `NOT NULL`, `UNIQUE` constraints
  - Supports multiple index types in `CREATE INDEX` statements: `HASH`, `SORTEDARRAY`, `BTREE` (default), `TTREE`
  - Performs syntax and semantic checks at compile time with clear error messages
  - Generates strongly typed Rust structs with field names and types strictly corresponding to DDL definitions
  - Generates static table metadata for database runtime use
  - Generates type-safe API prototypes: `insert`, `get_by_id`, `update`, `delete` functions
  - Zero runtime overhead, implemented using procedural macros
- **Rust Procedural Macro-based Zero-cost DDL Integration**:
  - Provides `MemdbTable` procedural macro supporting `#[derive(MemdbTable)]` syntax
  - Supports inline mode: write DDL directly in attributes
  - Supports file mode: associate external DDL files
  - Maps SQL constraints to Rust type system constraints, catching errors at compile time
  - Generated code is `#[repr(C)]` with memory layout identical to handwritten code
- **SQL Query Support**:
  - Supports standard SQL SELECT statements to query data in memory database
  - Supports basic queries, conditional queries, sorting, and LIMIT constraints
  - Supports comparison operators: `=`, `!=`, `<`, `<=`, `>`, `>=`
  - Provides user-friendly result set interface, supporting iterative access and field retrieval
- **Runtime DDL Configuration API**:
  - Trait-based DDL executor design with `DdlExecutor` trait
  - Supports runtime table and index creation
  - Supports DDL operations via SQL statements: `CREATE TABLE`, `CREATE INDEX`
  - Supports multiple index type configurations
  - Memory allocator abstraction, supporting custom memory allocation strategies
- **SQL Export Functionality**:
  - Supports exporting complete DDL files (table structures and index definitions)
  - Supports exporting table data as SQL INSERT statements
  - Output is compatible with SQLite3 syntax while preserving project-specific keywords
  - Supports writing export results to files or memory buffers
- **UDP-based Reliable Data Pub/Sub**:
  - Supports lightweight UDP-based data publish/subscribe mechanism
  - Performs CRC check on transmitted data to ensure data integrity
  - Supports unicast, broadcast, and multicast modes
  - Provides subscribe(topic_id, callback) and publish(topic_id, data) APIs
  - Supports NACK-based retransmission mechanism to improve data reliability
  - Supports heartbeat detection to automatically clean up inactive subscribers
  - Supports at least 16 concurrent subscribers per topic
  - Supports at least 32 different data topics
  - Latency from calling publish API to data entering network stack is less than 100 microseconds
  - Protocol header overhead is less than 10% of payload data

## Technical Characteristics

- **Zero External Dependencies**: No external library dependencies, supports no_std environments
- **Memory Allocation**: Supports both static and dynamic memory allocation with predictable memory usage suitable for resource-constrained embedded systems
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

### Basic Usage Example

```rust
#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::alloc::Layout;
use remdb::*;

// Define memory buffer
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

// Define table structure
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
        // Get database configuration
        let config = database!(tables: [users]);
        
        // Initialize memory allocator
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // Initialize platform abstraction layer
        platform::init_platform(platform::posix::get_posix_platform());
        
        // Calculate required memory size
        let table_size = MemoryTable::calculate_memory_size(config.tables[0]);
        let primary_index_size = PrimaryIndex::calculate_memory_size(
            config.tables[0],
            128, // Hash table size
            100  // Maximum index item count
        );
        let secondary_index_size = SecondaryIndex::calculate_memory_size(100);
        
        // Allocate memory
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
        
        // Create table and indices
        let mut table = MemoryTable::new(config.tables[0], table_ptr, status_ptr);
        let mut primary_index = PrimaryIndex::new(
            config.tables[0],
            hash_table_ptr,
            primary_index_items_ptr,
            128,
            100
        );
        let mut secondary_index = SecondaryIndex::new(config.tables[0], secondary_index_items_ptr, 100);
        
        // Initialize table and index arrays
        static mut TABLES: [Option<MemoryTable>; 1] = [None; 1];
        static mut PRIMARY_INDICES: [Option<PrimaryIndex>; 1] = [None; 1];
        static mut SECONDARY_INDICES: [Option<SecondaryIndex>; 1] = [None; 1];
        
        TABLES[0] = Some(table);
        PRIMARY_INDICES[0] = Some(primary_index);
        SECONDARY_INDICES[0] = Some(secondary_index);
        
        // Initialize global database
        let db = init_global_db(
            config,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        ).unwrap();
        
        // Use database...
    }
}

### SQL Query Example

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

### Low Power Mode Usage Example

```rust
// Define database with low power mode support
remdb::database!(
    TEST_DB,
    tables: [
        TEST_TABLE
    ],
    low_power: true,
    low_power_max_records: 100
);

// Initialize database
let db = remdb::init_global_db(
    &TEST_DB,
    &mut tables,
    &mut primary_indices,
    &mut secondary_indices
).unwrap();

// Enter low power mode
db.enter_low_power_mode().unwrap();

// Check current low power mode status
let is_low_power = db.is_low_power_mode();

// Insert records in low power mode
for i in 0..150 {
    match db.get_table_mut(0).unwrap().insert(record_data) {
        Ok(id) => println!("Inserted successfully, record ID: {}", id),
        Err(e) => println!("Insertion failed, error: {:?}", e),
    }
}

// Exit low power mode
db.exit_low_power_mode().unwrap();
```

### DDL Macro Usage Example

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
    
    // Test database configuration
    println!("Database tables count: {}", DATABASE.tables.len());
    
    // Test API functions (placeholder implementation)
    // user::insert(&mut db, user);
    // let result = user::get_by_id(&db, 1);
}
```

### Runtime DDL Configuration API Example

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
    
    // Create index using SQL statement
    let result = db.sql_query(
        "CREATE INDEX idx_product_name ON products (name) USING BTree;"
    );
}
```

### SQL Export Functionality Example

```rust
// Export DDL (table structures and indexes) to file
let result = db.export_ddl("./exported_ddl.sql");
if result.is_ok() {
    println!("DDL exported successfully!");
}

// Export table data to file
let result = db.export_data("./exported_data.sql");
if result.is_ok() {
    println!("Data exported successfully!");
}

// Export data for specific tables
// Parameters: filename, table name (optional, None means export all tables)
let result = db.export_data_with_table("./exported_users.sql", Some("users"));
if result.is_ok() {
    println!("Users table data exported successfully!");
}
```

### Publish/Subscribe Functionality Example

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

#### File Mode Usage Example

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

Check compilation in baremetal environment:

```bash
cargo check --no-default-features --features=baremetal
```

### Running Tests in Baremetal Environment

Directly running `cargo test` in baremetal environment will fail because the test framework depends on the std library. However, you can verify the correctness of the code in baremetal environment through the following steps:

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
- `generate_snapshot.rs`: Snapshot generation example demonstrating how to generate and use snapshots
- `sql_query.rs`: SQL query example demonstrating how to use SQL to query the in-memory database
- `ddl_example.rs`: DDL example demonstrating how to define tables and indexes using DDL macros
- `ddl_full_example.rs`: Complete DDL example demonstrating more complex DDL definitions
- `ddl_runtime_example.rs`: Runtime DDL configuration example demonstrating how to use the runtime DDL API
- `pubsub_example.rs`: Pub/Sub example demonstrating how to use the UDP-based reliable data publish/subscribe functionality

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
│   └── pubsub/
│       ├── mod.rs          # Pub/Sub module entry
│       ├── protocol.rs     # Protocol frame definition and parsing
│       ├── udp.rs          # Cross-platform UDP socket encapsulation
│       ├── subscriber.rs   # Subscriber management
│       ├── publisher.rs    # Publisher management
│       └── crc32.rs       # CRC32 check implementation
├── examples/
│   ├── basic_usage.rs      # Basic usage example
│   ├── low_power_mode.rs   # Low power mode example
│   ├── incremental_snapshot.rs # Incremental snapshot example
│   └── generate_snapshot.rs     # Snapshot generation example
├── tests/
│   ├── unit/
│   │   ├── memory_test.rs  # Memory management unit tests
│   │   └── table_test.rs   # Table operation unit tests
├── Cargo.toml              # Project configuration
├── Cargo.lock              # Dependency lock file
├── PLAN.md                 # Project plan
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
- Add performance monitoring in low power mode
- Implement adaptive low power mode that automatically switches based on system load
- Complete runtime DDL configuration API, supporting full table and index creation functionality
- Support DROP TABLE and ALTER TABLE statements
- Implement more flexible memory allocation strategies
- Optimize performance of runtime DDL operations
- Support more complex index configuration options
