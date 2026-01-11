use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};
#[cfg(feature = "pubsub")]
use remdb::pubsub::topics::*;

// 主题ID定义
const WAL_TOPIC_ID: u16 = 1;
const TABLES_TOPIC_ID: u16 = 2;
const METRICS_TOPIC_ID: u16 = 3;
const HEALTH_STATUS_TOPIC_ID: u16 = 4;

fn main() {
    println!("Starting PubSub Test Server...");
    
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
        heartbeat_interval: Duration::from_secs(5),
        frame_pool_size: 128,
    };
    
    // 创建发布/订阅实例
    let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
    pubsub.init().expect("Failed to initialize PubSub");
    
    // 注册所有预定义主题
    #[cfg(feature = "pubsub")] {
        pubsub.register_topic(WAL_TOPIC, WAL_TOPIC_ID).expect("Failed to register WAL topic");
        pubsub.register_topic(TABLES_TOPIC, TABLES_TOPIC_ID).expect("Failed to register tables topic");
        pubsub.register_topic(METRICS_TOPIC, METRICS_TOPIC_ID).expect("Failed to register metrics topic");
        pubsub.register_topic(HEALTH_STATUS_TOPIC, HEALTH_STATUS_TOPIC_ID).expect("Failed to register health status topic");
    }
    
    // 注册示例表内容主题
    let test_table_topic = format!("table.test_table");
    pubsub.register_topic("table.test_table", 12).expect("Failed to register test_table content topic");
    
    println!("PubSub test server started successfully!");
    println!("Listening on UDP port 5555");
    println!("Topics available:");
    println!("- WAL (ID: {}) - All WAL operations", WAL_TOPIC_ID);
    println!("- TABLES (ID: {}) - Table creation/deletion events", TABLES_TOPIC_ID);
    println!("- METRICS (ID: {}) - Database metrics", METRICS_TOPIC_ID);
    println!("- HEALTH_STATUS (ID: {}) - Health status updates", HEALTH_STATUS_TOPIC_ID);
    println!("- table.test_table (ID: 12) - Table content changes");
    println!("- HEARTBEAT - Sent every 5 seconds");
    
    // 启动心跳发送线程
    let pubsub_clone = Arc::new(Mutex::new(pubsub));
    let running = Arc::new(Mutex::new(true));
    let running_clone = running.clone();
    let server_clone = pubsub_clone.clone();
    
    let _heartbeat_thread = thread::spawn(move || {
        let mut interval = Duration::from_secs(5);
        while *running_clone.lock().unwrap() {
            // 发送心跳帧
            match server_clone.lock().unwrap().publish(0, b"heartbeat") {
                Ok(_) => println!("Heartbeat sent"),
                Err(e) => println!("Failed to send heartbeat: {:?}", e),
            }
            thread::sleep(interval);
        }
    });
    
    // 启动WAL日志模拟发布线程（循环发布不同类型的WAL操作）
    let running_clone_wal = running.clone();
    let server_clone_wal = pubsub_clone.clone();
    
    #[cfg(feature = "pubsub")] {
        let _wal_thread = thread::spawn(move || {
            let mut interval = Duration::from_millis(1000);
            let mut log_id = 0;
            let wal_op_types = [
                "INSERT",
                "UPDATE",
                "DELETE",
                "TIMESERIES_INSERT",
                "COMMIT",
                "ABORT",
                "CHECKPOINT",
            ];
            
            while *running_clone_wal.lock().unwrap() {
                // 循环遍历所有WAL操作类型
                let op_type = wal_op_types[log_id % wal_op_types.len()];
                let wal_data = format!("WAL_LOG_{}: Operation={}, Table=test_table, ID={}, Data={}", log_id, op_type, log_id, format!("test_data_{}", log_id));
                
                // 发布到单一WAL主题
                match server_clone_wal.lock().unwrap().publish(WAL_TOPIC_ID, wal_data.as_bytes()) {
                    Ok(_) => println!("Published WAL: {}", wal_data),
                    Err(e) => println!("Failed to publish WAL: {:?}", e),
                }
                
                log_id += 1;
                thread::sleep(interval);
            }
        });
    }
    
    // 启动表内容变更模拟发布线程
    let running_clone_table = running.clone();
    let server_clone_table = pubsub_clone.clone();
    
    let _table_thread = thread::spawn(move || {
        let mut interval = Duration::from_millis(2000);
        let mut record_id = 0;
        #[cfg(feature = "pubsub")]
        let test_table_topic = get_table_content_topic("test_table");
        #[cfg(not(feature = "pubsub"))]
        let test_table_topic = "table.test_table";
        
        while *running_clone_table.lock().unwrap() {
            let table_data = format!("TABLE_CONTENT_{}: Table=test_table, ID={}, Column1=value_{}, Column2={}", record_id, record_id, record_id, record_id * 2);
            let table_topic_id = 12; // 预定义的表内容主题ID
            
            match server_clone_table.lock().unwrap().publish(table_topic_id, table_data.as_bytes()) {
                Ok(_) => println!("Published {}: {}", test_table_topic, table_data),
                Err(e) => println!("Failed to publish {}: {:?}", test_table_topic, e),
            }
            record_id += 1;
            thread::sleep(interval);
        }
    });
    
    // 启动表创建/删除事件发布线程
    let running_clone_tables = running.clone();
    let server_clone_tables = pubsub_clone.clone();
    
    let _tables_thread = thread::spawn(move || {
        let mut interval = Duration::from_millis(3000);
        let mut table_id = 0;
        
        while *running_clone_tables.lock().unwrap() {
            let is_create = table_id % 2 == 0;
            let tables_event = if is_create {
                format!("CREATE:table=table_{},id={},fields=3", table_id, table_id)
            } else {
                format!("DELETE:table=table_{},id={}", table_id - 1, table_id - 1)
            };
            
            match server_clone_tables.lock().unwrap().publish(TABLES_TOPIC_ID, tables_event.as_bytes()) {
                Ok(_) => println!("Published TABLES: {}", tables_event),
                Err(e) => println!("Failed to publish TABLES: {:?}", e),
            }
            table_id += 1;
            thread::sleep(interval);
        }
    });
    
    // 启动指标发布线程
    let running_clone_metrics = running.clone();
    let server_clone_metrics = pubsub_clone.clone();
    
    let _metrics_thread = thread::spawn(move || {
        let mut interval = Duration::from_millis(4000);
        let mut metric_id = 0;
        
        while *running_clone_metrics.lock().unwrap() {
            let metrics_data = format!(
                r#"{{"total_memory":1048576,"used_memory":{},"read_ops":{},"write_ops":{},"delete_ops":{},"update_ops":{},"cache_hits":{},"cache_misses":{},"cache_hit_rate":85.5,"index_lookups":{},"index_inserts":{},"index_deletes":{},"transactions":{},"committed_transactions":{},"rolled_back_transactions":{}}}"#,
                500000 + metric_id * 1000,
                1000 + metric_id * 10,
                2000 + metric_id * 20,
                500 + metric_id * 5,
                1500 + metric_id * 15,
                8000 + metric_id * 80,
                1300 + metric_id * 13,
                500 + metric_id * 5,
                300 + metric_id * 3,
                100 + metric_id,
                150 + metric_id * 15,
                120 + metric_id * 12,
                50 + metric_id * 5
            );
            
            match server_clone_metrics.lock().unwrap().publish(METRICS_TOPIC_ID, metrics_data.as_bytes()) {
                Ok(_) => println!("Published METRICS: {}", metrics_data),
                Err(e) => println!("Failed to publish METRICS: {:?}", e),
            }
            metric_id += 1;
            thread::sleep(interval);
        }
    });
    
    // 启动健康状态发布线程
    let running_clone_health = running.clone();
    let server_clone_health = pubsub_clone.clone();
    
    let _health_thread = thread::spawn(move || {
        let mut interval = Duration::from_millis(5000);
        let health_statuses = ["Healthy", "Warning", "Healthy", "Unhealthy", "Healthy"];
        let mut health_index = 0;
        
        while *running_clone_health.lock().unwrap() {
            let status = health_statuses[health_index % health_statuses.len()];
            let details = match status {
                "Healthy" => "数据库运行正常",
                "Warning" => "内存使用率较高",
                "Unhealthy" => "内存使用率过高",
                _ => "未知状态",
            };
            
            let health_data = format!(
                r#"{{"status":"{}","timestamp":{},"metrics":{{"total_memory":1048576,"used_memory":{},"read_ops":1000,"write_ops":2000,"delete_ops":500,"update_ops":1500,"cache_hits":8000,"cache_misses":1300,"cache_hit_rate":85.5,"index_lookups":500,"index_inserts":300,"index_deletes":100,"transactions":150,"committed_transactions":120,"rolled_back_transactions":30}},"details":"{}"}}
"#,
                status,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                500000 + health_index * 1000,
                details
            );
            
            match server_clone_health.lock().unwrap().publish(HEALTH_STATUS_TOPIC_ID, health_data.as_bytes()) {
                Ok(_) => println!("Published HEALTH_STATUS: {}", health_data.trim()),
                Err(e) => println!("Failed to publish HEALTH_STATUS: {:?}", e),
            }
            health_index += 1;
            thread::sleep(interval);
        }
    });
    
    // 运行1分钟后自动停止
    thread::sleep(Duration::from_secs(60));
    
    // 停止所有线程
    *running.lock().unwrap() = false;
    
    println!("PubSub test server stopped!");
}