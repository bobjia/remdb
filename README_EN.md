# remdb - Embedded In-Memory Database

[中文版](./README.md)

remdb is a lightweight embedded in-memory database designed for resource-constrained embedded systems, supporting no_std environments with predictable memory usage and high performance.

## Key Features

- **In-Memory Table Storage**: Efficient in-memory table implementation supporting insert, delete, query, and traversal operations
- **Indexing Mechanisms**:
  - Hash-based primary key index providing O(1) query performance
  - Ordered array-based secondary index supporting range queries
- **Transaction Support**: Basic transaction management including begin, commit, and rollback operations
- **Memory Management**:
  - Static memory allocator with no dynamic memory allocation
  - Fixed-size block memory pool enabling efficient memory management
- **Platform Abstraction Layer**: Supports both POSIX and baremetal environments
- **Compile-time Configuration**: Table and database configuration implemented via macros for performance optimization

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

## Examples

Check the examples directory for sample code:

- `basic_usage.rs`: Basic usage example demonstrating table definition, insertion, query, and transaction operations

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
│   └── basic_usage.rs      # Basic usage example
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
4. Currently only basic transaction functionality is supported, no complex transaction isolation levels

## Future Plans

- Support more data types
- Implement more advanced transaction isolation levels
- Add persistence support
- Optimize memory usage
- Provide more index types
- Add more examples and documentation
