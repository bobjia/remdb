use remdb::config::{DbConfig, WALConfig, DefaultMemoryAllocator};
use remdb::memory::allocator;
use remdb::time_series::table::TimeSeriesConfig;
use remdb::{RemDb, Result};

// 定义数据库内存区域
static mut DB_MEMORY: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024];

// 静态内存分配器实例
static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

fn main() -> Result<()> {
    // 初始化全局内存分配器
    unsafe {
        allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;
    }

    // 定义数据库配置
    let config = Box::leak(Box::new(DbConfig {
        tables: vec![],                    // 空的数据库配置
        total_memory: 32 * 1024 * 1024, // 32MB，与全局缓冲区大小一致
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10000,
        memory_allocator: &ALLOCATOR, // 使用默认内存分配器
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60000,         // 60秒
            log_file_size_limit: 16 * 1024 * 1024, // 16MB
            log_prealloc_size: 4 * 1024 * 1024,    // 4MB
            log_segment_size: 16 * 1024 * 1024,    // 16MB
            retained_checkpoints: 2,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
        },
        time_series_defaults: TimeSeriesConfig {
            partition_duration_secs: 3600,        // 1小时
            retention_period_secs: 7 * 24 * 3600, // 7天
            compression: remdb::time_series::compression::CompressionType::None,
            max_partitions: 100,
        },
        #[cfg(feature = "ha")]
        ha_config: None,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
    }));

    // 初始化数据库
    let mut db = RemDb::new(config);

    // 初始化数据库
    db.init()?;

    // 使用SQL创建包含向量字段的表
    let create_sql = r#"CREATE TABLE products (
        id INT32 PRIMARY KEY,
        name TEXT,
        embedding VECTOR(4) WITH DISTANCE=IP
    )"#;
    db.sql_query(create_sql)?;
    println!("成功创建包含向量字段的表！");

    // 通过SQL插入向量数据
    let insert_sql = r#"INSERT INTO products (id, name, embedding) VALUES
        (1, 'product1', '[0.1, 0.2, 0.3, 0.4]'),
        (2, 'product2', '[1.0, 0.9, 0.8, 0.7]')
    "#;
    db.sql_query(insert_sql)?;
    println!("成功插入向量数据！");

    // 查询插入的数据
    let select_sql = "SELECT id, name FROM products";
    let result = db.sql_query(select_sql)?;
    println!("\n查询结果：");
    println!("查询成功，返回 {} 行数据", result.rows.len());
    for (i, row) in result.rows.iter().enumerate() {
        println!("行 {}: id={:?}, name={:?}", i+1, row.values[0], row.values[1]);
    }

    // 向量相似性查询 - 使用内积距离操作符 <#> (越大越相似)
    let ip_sql = "SELECT id, name, embedding <#> '[0.2, 0.3, 0.4, 0.5]' AS ip_similarity FROM products ORDER BY ip_similarity DESC LIMIT 2";
    let ip_result = db.sql_query(ip_sql)?;
    println!("\n内积距离相似性查询结果：");
    println!("查询成功，返回 {} 行数据", ip_result.rows.len());
    for (i, row) in ip_result.rows.iter().enumerate() {
        println!("行 {}: id={:?}, name={:?}, ip_similarity={:?}", i+1, row.values[0], row.values[1], row.values[2]);
    }

    // 向量相似性查询 - 使用余弦相似度操作符 <=> (越大越相似)
    let cosine_sql = "SELECT id, name, embedding <=> '[0.2, 0.3, 0.4, 0.5]' AS cosine_similarity FROM products ORDER BY cosine_similarity DESC LIMIT 2";
    let cosine_result = db.sql_query(cosine_sql)?;
    println!("\n余弦相似度查询结果：");
    println!("查询成功，返回 {} 行数据", cosine_result.rows.len());
    for (i, row) in cosine_result.rows.iter().enumerate() {
        println!("行 {}: id={:?}, name={:?}, cosine_similarity={:?}", i+1, row.values[0], row.values[1], row.values[2]);
    }

    // 向量相似性查询 - 使用L2距离操作符 <-> (越小越相似)
    let l2_sql = "SELECT id, name, embedding <-> '[0.2, 0.3, 0.4, 0.5]' AS l2_distance FROM products ORDER BY l2_distance ASC LIMIT 2";
    let l2_result = db.sql_query(l2_sql)?;
    println!("\nL2距离相似性查询结果：");
    println!("查询成功，返回 {} 行数据", l2_result.rows.len());
    for (i, row) in l2_result.rows.iter().enumerate() {
        println!("行 {}: id={:?}, name={:?}, l2_distance={:?}", i+1, row.values[0], row.values[1], row.values[2]);
    }

    // 初始化索引构建线程池
    remdb::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功！");

    // 创建向量索引
    let create_index_sql = "CREATE INDEX idx_products_embedding ON products (embedding) USING HNSW WITH (M=16, ef_construction=200)";
    db.sql_query(create_index_sql)?;
    println!("成功创建向量索引！");

    // 向量功能说明
    println!("\n向量功能支持情况：");
    println!("✓ 支持向量字段定义: VECTOR(dimension)");
    println!("✓ 支持距离度量指定: WITH DISTANCE=L2/COSINE/IP/INNER_PRODUCT");
    println!("✓ 支持向量数据插入: INSERT INTO table (vector_col) VALUES ([1.0, 2.0, ...])");
    println!("✓ 支持向量相似性查询: SELECT * FROM table ORDER BY vector_col <-> '[1.0, 2.0, ...]' LIMIT k");
    println!("✓ 支持向量索引: HNSW/HNSW_SQ/HNSW_BQ/IVF/IVF_PQ");

    println!("\n示例SQL语法：");
    println!("1. 创建向量表");
    println!("   CREATE TABLE products (id INT32 PRIMARY KEY, embedding VECTOR(64) WITH DISTANCE=IP)");
    println!("   CREATE TABLE products_l2 (id INT32 PRIMARY KEY, embedding VECTOR(64) WITH DISTANCE=L2)");
    println!("   CREATE TABLE products_cosine (id INT32 PRIMARY KEY, embedding VECTOR(64) WITH DISTANCE=COSINE)");
    println!();
    println!("2. 插入向量数据");
    println!("   INSERT INTO products (id, embedding) VALUES (1, '[1.0, 2.0, 3.0, ...]')");
    println!();
    println!("3. 向量相似性查询");
    println!("   SELECT * FROM products ORDER BY embedding <-> '[1.0, 2.0, ...]' LIMIT 5  -- L2距离");
    println!("   SELECT * FROM products ORDER BY embedding <#> '[1.0, 2.0, ...]' DESC LIMIT 5  -- 内积");
    println!("   SELECT * FROM products ORDER BY embedding <=> '[1.0, 2.0, ...]' DESC LIMIT 5  -- 余弦相似度");
    println!();
    println!("4. 混合查询");
    println!("   SELECT * FROM products WHERE price < 100 AND embedding <=> '[1.0, 2.0, ...]' > 0.8 LIMIT 5");
    println!();
    println!("5. 创建向量索引");
    println!("   CREATE INDEX idx_vec ON table (vector_col) USING HNSW WITH (M=16, ef_construction=200)");
    println!("   CREATE INDEX idx_vec_sq ON table (vector_col) USING HNSW_SQ WITH (M=16, ef_construction=200, DISTANCE=COSINE)");
    println!("   CREATE INDEX idx_vec_bq ON table (vector_col) USING HNSW_BQ WITH (M=16, ef_construction=200, DISTANCE=IP)");
    println!("   CREATE INDEX idx_vec_ivf ON table (vector_col) USING IVF WITH (nlist=128, DISTANCE=L2)");
    println!("   CREATE INDEX idx_vec_ivfpq ON table (vector_col) USING IVF_PQ WITH (nlist=128, nprobe=8, M=8, nbits=8)");

    println!("\n向量表示例运行完成！");

    Ok(())
}
