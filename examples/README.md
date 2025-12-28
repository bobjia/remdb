# C API Example Compilation and Execution Guide

This guide explains how to compile and run the `c_api_example.c` example program, which demonstrates the usage of RemDB C API.

## Prerequisites

1. A C compiler (gcc, clang, or MSVC)
2. RemDB library compiled with C API support
3. RemDB header files

## Compilation Steps

### 1. Ensure RemDB is compiled with C API support

```bash
cargo build --release --features c-api
```

### 2. Compile the example program

#### Using GCC (Linux/macOS)

```bash
cd examples
gcc -o c_api_example c_api_example.c -lremdb -L../target/release -I../include
```

#### Using MSVC (Windows)

```bash
cd examples
cl /I../include c_api_example.c ..\target\release\remdb.lib
```

## Execution

### 1. Run the compiled program

```bash
# Linux/macOS
./c_api_example

# Windows
c_api_example.exe
```

### 2. Expected Output

The program should produce output similar to the following:

```
RemDB C API Example
=====================

Inserting records...
Inserted user: 1, Alice, 25
Inserted user: 2, Bob, 30
Inserted user: 3, Charlie, 35

Querying records...
Retrieved user: 2, Bob, 30

Updating record...
Updated user 2 to: 2, Robert, 31
Verified updated user: 2, Robert, 31

Transaction example...
Transaction started successfully
Inserted user 4 in transaction: 4, David, 28
Transaction committed successfully

Getting record count...
Current record count: 4

Snapshot management...
Snapshot saved successfully to 'example_snapshot'
Deleting user 3...
Deleted user 3
Record count after deletion: 3
Restoring snapshot...
Snapshot restored successfully
Record count after restoration: 4

Health check...
Health status: Healthy
Health details: 数据库运行正常
Memory usage: 48 / 1048576 bytes

Metrics dump...
RemDB Metrics:
  Total Memory: 1048576 bytes
  Used Memory: 48 bytes
  Read Operations: 2
  Write Operations: 5
  Delete Operations: 1
  Update Operations: 1
  Cache Hits: 0
  Cache Misses: 0
  Index Lookups: 0
  Index Inserts: 0
  Index Deletes: 0
  Transactions: 1
  Committed Transactions: 1
  Rolled Back Transactions: 0
  Start Time: 0

Getting snapshot version...
Current snapshot version: 1

Low power mode example...
Current low power mode status: Disabled
Entered low power mode
Updated low power mode status: Enabled
Exited low power mode
Final low power mode status: Disabled

RemDB C API Example completed successfully!
========================================
```

## Troubleshooting

### Compilation Errors

**Problem**: `undefined reference to xxx`
**Solution**: Ensure that RemDB library is properly linked and that the library path is correct.

**Problem**: `remdb.h: No such file or directory`
**Solution**: Ensure that the include path is correct and that `remdb.h` is present in the specified directory.

### Runtime Errors

**Problem**: `Failed to initialize database: error code 1`
**Solution**: Check if the database configuration is correct, especially the memory size and table definitions.

**Problem**: `Failed to insert record: error code 3`
**Solution**: This indicates a duplicate key error. Ensure that each record has a unique primary key.

**Problem**: `Failed to save snapshot: error code 9`
**Solution**: Check if the program has write permission to the specified snapshot file path.

## Understanding the Example

The example program demonstrates the following RemDB C API features:

1. **Database Initialization**: How to initialize the RemDB database with table definitions
2. **CRUD Operations**: How to insert, query, update, and delete records
3. **Transactions**: How to use transactions to ensure data consistency
4. **Snapshot Management**: How to save and restore database snapshots, including incremental snapshots
5. **Health Monitoring**: How to check the health status of the database
6. **Metrics Collection**: How to collect and display database metrics
7. **Low Power Mode**: How to enable and disable low power mode
8. **Table Operations**: How to get table information and records by name

## Customizing the Example

You can modify the example program to test different RemDB features:

1. **Change Database Configuration**: Modify the `RemDbConfig` structure to change memory size, table definitions, etc.
2. **Add More Tables**: Define additional `RemDbTableDef` structures and add them to the configuration.
3. **Test Different Isolation Levels**: Modify the `remdb_begin_transaction` call to test different isolation levels.
4. **Test Incremental Snapshots**: Use `remdb_save_incremental_snapshot` to test incremental snapshot functionality.

## Conclusion

The `c_api_example.c` program provides a comprehensive example of how to use the RemDB C API. By compiling and running this example, you can verify that the RemDB C API is working correctly on your system.