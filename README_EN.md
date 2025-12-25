# remdb - Embedded In-Memory Database

[中文版](./README.md)

remdb is a lightweight embedded in-memory database designed for resource-constrained embedded systems, supporting no_std environments with predictable memory usage and high performance.

## Key Features

- **In-Memory Table Storage**: Efficient in-memory table implementation supporting insert, delete, query, and traversal operations
- **Indexing Mechanisms**:
  - Hash-based primary key index providing O(1) query performance
  - Ordered array-based secondary index supporting range queries
- **Transaction Support**: Complete ACID transaction support, including:
  - Atomicity: Transactions are either fully committed or fully rolled back
  - Consistency: Ensures data integrity and correctness
  - Isolation: Supports multiple isolation levels (Read Uncommitted, Read Committed, Repeatable Read, Serializable)
  - Durability: Ensures data persistence through Write-Ahead Logging (WAL)
  - Supports record-level locking (shared locks and exclusive locks)
  - Supports transaction logging and crash recovery
- **Memory Management**:
  - Static memory allocator with no dynamic memory allocation
  - Fixed-size block memory pool enabling efficient memory management
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
  - Supports core SQLite3 DDL syntax: `CREATE TABLE`, column definitions, `PRIMARY KEY`, `NOT NULL`, `UNIQUE` constraints
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

## Technical Characteristics

- **Zero External Dependencies**: No external library dependencies, supports no_std environments
- **Static Memory Allocation**: Predictable memory usage suitable for resource-constrained embedded systems
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

// Define table using inline DDL
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE user (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, active BOOLEAN);")]
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

#### File Mode Usage Example

```rust
use remdb_macros::MemdbTable;

// Define tables using external DDL file
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
// CREATE TABLE product (
//     id INTEGER PRIMARY KEY,
//     name TEXT NOT NULL,
//     price REAL NOT NULL,
//     category TEXT
// );
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
│   ├── memory/
│   │   ├── allocator.rs    # Static memory allocator
│   │   ├── pool.rs         # Memory pool
│   │   └── mod.rs
│   └── platform/
│       ├── mod.rs          # Platform abstraction layer definition
│       ├── posix.rs        # POSIX platform implementation
│       └── baremetal.rs    # Baremetal platform implementation
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
