extern crate alloc;

use remdb::*;

static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152];

remdb::database!(
    TEST_DB,
    tables: [],
    low_power: true,
    low_power_max_records: 50
);

fn main() {
    unsafe {
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        #[cfg(feature = "posix")]
        remdb::platform::init_platform(remdb::platform::posix::get_posix_platform());

        let db = remdb::init_global_db(&TEST_DB).unwrap();

        println!("Testing CREATE TABLE WITH CONFIGURATION (max_records=100)");
        println!("Low power mode max_records: {:?}", TEST_DB.low_power_max_records);
        println!("Expected effective max_records: 50 (min of 100 and 50)");

        let sql = "CREATE TABLE my_memory_table (
            id INT AUTO_INCREMENT PRIMARY KEY,
            data VARCHAR(255)
        ) WITH CONFIGURATION (max_records=100);";

        match db.sql_query(sql) {
            Ok(result) => {
                println!("Table created successfully!");
                println!("Result: OK");
            }
            Err(e) => {
                println!("Error creating table: {:?}", e);
            }
        }

        match db.get_table_and_secondary_index_mut_by_name("my_memory_table") {
            Ok((table, _)) => {
                println!("Table max_records: {}", table.max_records());
                println!("Expected: 50 (low power mode limit)");
            }
            Err(e) => println!("Error getting table: {:?}", e),
        }

        println!("\nTesting CREATE TABLE without max_records configuration");
        let sql2 = "CREATE TABLE default_table (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100)
        );";

        match db.sql_query(sql2) {
            Ok(result) => {
                println!("Table created successfully!");
                println!("Result: OK");
            }
            Err(e) => {
                println!("Error creating table: {:?}", e);
            }
        }

        match db.get_table_and_secondary_index_mut_by_name("default_table") {
            Ok((table, _)) => {
                println!("Table max_records: {}", table.max_records());
                println!("Expected: 50 (low power mode default)");
            }
            Err(e) => println!("Error getting table: {:?}", e),
        }

        println!("\nAll tests completed!");
    }
}
