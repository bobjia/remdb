//! SQL Vector Operations Example
//!
//! This example demonstrates RemDB's vector operation features:
//! - Vector field definition
//! - Vector distance operators (<->, <#>, <=>)
//! - Vector index creation
//! - Vector similarity search

use remdb::config::{DbConfig, DefaultMemoryAllocator, WALConfig};
use remdb::index::builder::init_index_build_thread_pool;
use remdb::{RemDb, Result};

static mut DB_MEMORY: [u8; 16 * 1024 * 1024] = [0; 16 * 1024 * 1024];

static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

fn main() -> Result<()> {
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;
    }

    let config = Box::leak(Box::new(DbConfig {
        tables: vec![],
        total_memory: 16 * 1024 * 1024,
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "ha")]
        ha_config: None,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,

        model_worker_config: Default::default(),
    }));

    let mut db = RemDb::new(config);
    db.init()?;
    init_index_build_thread_pool(2);

    println!("=== SQL Vector Operations Example ===\n");

    // 1. Create table with vector field
    println!("1. Create table with vector field");
    db.sql_query("CREATE TABLE documents (id INT32 PRIMARY KEY, title TEXT, content TEXT, embedding VECTOR(128) WITH DISTANCE=L2)")?;
    println!("   Created table: documents (with 128-dim vector field)");

    db.sql_query("CREATE TABLE products (id INT32 PRIMARY KEY, name TEXT, price REAL, features VECTOR(64) WITH DISTANCE=COSINE)")?;
    println!("   Created table: products (with 64-dim vector field)");

    // 2. Insert vector data
    println!("\n2. Insert vector data");

    for i in 1..=5 {
        let embedding: Vec<String> = (0..128)
            .map(|j| format!("{:.4}", (i as f64 * 0.1 + j as f64 * 0.01)))
            .collect();
        let embedding_str = format!("[{}]", embedding.join(", "));
        let sql = format!(
            "INSERT INTO documents VALUES ({}, 'Doc {}', 'Content for document {}', '{}')",
            i, i, i, embedding_str
        );
        db.sql_query(&sql)?;
    }
    println!("   Inserted 5 document records");

    for i in 1..=5 {
        let features: Vec<String> = (0..64)
            .map(|j| format!("{:.4}", (i as f64 * 0.2 + j as f64 * 0.02)))
            .collect();
        let features_str = format!("[{}]", features.join(", "));
        let sql = format!(
            "INSERT INTO products VALUES ({}, 'Product {}', {}, '{}')",
            i,
            i,
            i as f64 * 10.0,
            features_str
        );
        db.sql_query(&sql)?;
    }
    println!("   Inserted 5 product records");

    // 3. Create vector index
    println!("\n3. Create vector index");

    let result = db.sql_query("CREATE INDEX idx_doc_embedding ON documents (embedding) USING HNSW WITH (M=16, ef_construction=200, DISTANCE=L2)");
    match result {
        Ok(_) => println!("   Created HNSW index: idx_doc_embedding"),
        Err(e) => println!("   Index creation: {:?}", e),
    }

    // 4. Vector distance operator - L2 distance (<->)
    println!("\n4. Vector distance operator - L2 distance (<->)");

    let query_vec: Vec<String> = (0..128)
        .map(|j| format!("{:.4}", j as f64 * 0.01))
        .collect();
    let query_str = format!("[{}]", query_vec.join(", "));

    let result = db.sql_query(&format!(
        "SELECT id, title, embedding <-> '{}' AS distance FROM documents ORDER BY distance LIMIT 3",
        query_str
    ));
    match result {
        Ok(r) => {
            println!("   L2 distance search results:");
            println!("{}", r.to_string());
        }
        Err(e) => println!("   Query error: {:?}", e),
    }

    // 5. Vector distance operator - Cosine similarity (<=>)
    println!("\n5. Vector distance operator - Cosine similarity (<=>)");

    let query_vec2: Vec<String> = (0..64).map(|j| format!("{:.4}", j as f64 * 0.02)).collect();
    let query_str2 = format!("[{}]", query_vec2.join(", "));

    let result = db.sql_query(&format!(
        "SELECT id, name, features <=> '{}' AS similarity FROM products ORDER BY similarity DESC LIMIT 3",
        query_str2
    ));
    match result {
        Ok(r) => {
            println!("   Cosine similarity search results:");
            println!("{}", r.to_string());
        }
        Err(e) => println!("   Query error: {:?}", e),
    }

    // 6. Vector search + scalar filter
    println!("\n6. Vector search + scalar filter");

    let result = db.sql_query(&format!(
        "SELECT id, name, price, features <=> '{}' AS similarity FROM products WHERE price < 40.0 ORDER BY similarity DESC LIMIT 5",
        query_str2
    ));
    match result {
        Ok(r) => {
            println!("   Similarity search for products with price < 40:");
            println!("{}", r.to_string());
        }
        Err(e) => println!("   Query error: {:?}", e),
    }

    // 7. Show table structure
    println!("\n7. Show table structure");
    let result = db.sql_query("DESCRIBE documents")?;
    println!("   documents table structure:");
    println!("{}", result.to_string());

    println!("\n=== SQL Vector Operations Example Complete ===");
    Ok(())
}
