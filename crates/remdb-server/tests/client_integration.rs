//! End-to-end integration tests: real TCP client talks to an in-process
//! remdb-server over the length-prefixed protobuf frame protocol.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use prost::Message as _;

use remdb_server::handler::SharedDb;
use remdb_server::pb;
use remdb_server::pubsub::PubSubManager;
use remdb_server::server::serve;

static PLATFORM_INIT: std::sync::Once = std::sync::Once::new();
static ALLOC: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;

/// Bootstrap a remdb instance with the feature-unified DbConfig (pubsub + ha),
/// bind a server on the given port, and spawn the accept loop.
fn start_server(port: u16, wal_dir: &'static str) -> std::thread::JoinHandle<()> {
    PLATFORM_INIT.call_once(|| {
        remdb::platform::init_platform(remdb_platform_posix::get_posix_platform());
    });
    let config = Box::leak(Box::new(remdb::config::DbConfig {
        tables: &[],
        total_memory: 8 * 1024 * 1024,
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100_000,
        memory_allocator: &ALLOC,
        wal_config: remdb::config::WALConfig {
            log_path: wal_dir,
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60_000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 4 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
        },
        time_series_defaults: remdb::time_series::table::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 3600,
            compression: remdb::time_series::compression::CompressionType::None,
            max_partitions: 10,
        },
        pubsub_config: None,
        ha_config: None,
    }));
    assert!(remdb::config::validate_config(config));
    let mut db = remdb::RemDb::new(config);
    db.init().expect("init");
    let db: SharedDb = Arc::new(Mutex::new(db));
    let pubsub = Arc::new(PubSubManager::new());
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).expect("bind");
    std::thread::spawn(move || {
        serve(listener, db, pubsub).expect("serve");
    })
}

/// Pick a free ephemeral port by binding to :0, reading the port, then dropping.
fn ephemeral_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    l.local_addr().expect("addr").port()
}

struct Client {
    stream: TcpStream,
}

impl Client {
    fn connect(port: u16) -> Self {
        Client {
            stream: TcpStream::connect(("127.0.0.1", port)).expect("connect"),
        }
    }

    /// Send one request frame and read the corresponding response frame.
    fn round_trip(&mut self, req: pb::Request) -> pb::Response {
        let payload = req.encode_to_vec();
        let frame = remdb_server::protocol::encode_frame(&payload);
        self.stream.write_all(&frame).expect("write frame");

        let mut prefix = [0u8; 4];
        self.stream.read_exact(&mut prefix).expect("read prefix");
        let len = u32::from_be_bytes(prefix) as usize;
        let mut body = vec![0u8; len];
        self.stream.read_exact(&mut body).expect("read body");
        pb::Response::decode(body.as_slice()).expect("decode response")
    }
}

#[test]
fn ping_round_trip() {
    let port = ephemeral_port();
    let _h = start_server(port, "/tmp/remdb-srv-test-ping");

    let mut client = Client::connect(port);
    let resp = client.round_trip(pb::Request {
        request_id: 7,
        op: Some(pb::request::Op::Ping(pb::PingRequest {
            client_version: "test".into(),
        })),
    });

    assert_eq!(resp.request_id, 7);
    let status = resp.status.expect("status present");
    assert_eq!(status.code, 0, "ping should succeed (OK)");
    assert!(status.message.is_empty());
}

#[test]
fn ddl_insert_select_and_error_flow() {
    let port = ephemeral_port();
    let _h = start_server(port, "/tmp/remdb-srv-test-flow");

    let mut client = Client::connect(port);

    // a) DDL: create a table via the Ddl op.
    let ddl = client.round_trip(pb::Request {
        request_id: 1,
        op: Some(pb::request::Op::Ddl(pb::DdlRequest {
            sql: "CREATE TABLE users (id INT, name TEXT)".into(),
        })),
    });
    let ddl_status = ddl.status.expect("ddl status present");
    assert_eq!(
        ddl_status.code,
        0,
        "DDL should succeed, got message: {:?}",
        ddl_status.message
    );

    // b) INSERT via raw Query SQL.
    let ins = client.round_trip(pb::Request {
        request_id: 2,
        op: Some(pb::request::Op::Query(pb::QueryRequest {
            sql: "INSERT INTO users (id, name) VALUES (1, 'alice')".into(),
        })),
    });
    let ins_status = ins.status.expect("insert status present");
    assert_eq!(
        ins_status.code,
        0,
        "INSERT should succeed, got message: {:?}",
        ins_status.message
    );

    // b2) INSERT via structured Crud op (positional full-row VALUES).
    let strutt = client.round_trip(pb::Request {
        request_id: 5,
        op: Some(pb::request::Op::Crud(pb::CrudRequest {
            table: "users".into(),
            op: Some(pb::crud_request::Op::Insert(pb::Insert {
                values: vec![
                    pb::Value {
                        v: Some(pb::value::V::VInt(2)),
                    },
                    pb::Value {
                        v: Some(pb::value::V::VStr("bob".into())),
                    },
                ],
            })),
        })),
    });
    let strutt_status = strutt.status.expect("struct insert status present");
    assert_eq!(
        strutt_status.code,
        0,
        "structured INSERT should succeed, got message: {:?}",
        strutt_status.message
    );

    // c) SELECT via raw Query SQL and validate the payload (2 rows now).
    let sel = client.round_trip(pb::Request {
        request_id: 3,
        op: Some(pb::request::Op::Query(pb::QueryRequest {
            sql: "SELECT id, name FROM users".into(),
        })),
    });
    let sel_status = sel.status.expect("select status present");
    assert_eq!(
        sel_status.code,
        0,
        "SELECT should succeed, got message: {:?}",
        sel_status.message
    );
    let qr = match sel.payload {
        Some(pb::response::Payload::Query(qr)) => qr,
        other => panic!("expected Query payload, got {:?}", other),
    };
    // Lenient column count, but require a sane schema.
    assert!(!qr.columns.is_empty(), "expected at least one column");
    let cols: Vec<String> = qr.columns.iter().map(|c| c.name.clone()).collect();
    eprintln!("SELECT columns: {:?}", cols);
    assert!(
        qr.record_count >= 1,
        "expected at least one record, got {}",
        qr.record_count
    );
    // raw_data must be non-empty and, for fixed-size rows, match count*size.
    if qr.record_size > 0 {
        assert_eq!(
            qr.raw_data.len(),
            (qr.record_count as usize) * (qr.record_size as usize),
            "raw_data length must equal record_count * record_size"
        );
    } else {
        assert!(!qr.raw_data.is_empty() || qr.record_count == 0);
    }

    // d) ERROR: query a non-existent table -> non-zero status code.
    let err = client.round_trip(pb::Request {
        request_id: 4,
        op: Some(pb::request::Op::Query(pb::QueryRequest {
            sql: "SELECT * FROM nonexistent_table".into(),
        })),
    });
    let err_status = err.status.expect("error status present");
    assert_ne!(
        err_status.code,
        0,
        "querying a missing table should produce a non-zero error code"
    );
    eprintln!("error-path status code: {}", err_status.code);
}

#[test]
fn pubsub_subscribe_publish_event() {
    let port = ephemeral_port();
    let _h = start_server(port, "/tmp/remdb-srv-test-pubsub");

    // Subscriber client
    let mut sub = Client::connect(port);
    let sub_resp = sub.round_trip(pb::Request {
        request_id: 1,
        op: Some(pb::request::Op::Subscribe(pb::SubscribeRequest {
            topic: "sensors".into(),
        })),
    });
    let sub_status = sub_resp.status.expect("subscribe status present");
    assert_eq!(sub_status.code, 0, "subscribe should succeed");
    let sub_id = match sub_resp.payload {
        Some(pb::response::Payload::Subscribe(ref s)) => s.subscription_id,
        other => panic!("expected Subscribe payload, got {:?}", other),
    };
    assert!(sub_id > 0, "subscription id should be non-zero");

    // Publisher client
    let mut pub_ = Client::connect(port);
    let pub_resp = pub_.round_trip(pb::Request {
        request_id: 2,
        op: Some(pb::request::Op::Publish(pb::PublishRequest {
            topic: "sensors".into(),
            payload: vec![10, 20, 30],
        })),
    });
    let pub_status = pub_resp.status.expect("publish status present");
    assert_eq!(pub_status.code, 0, "publish should succeed");
    let count = match pub_resp.payload {
        Some(pb::response::Payload::Publish(ref p)) => p.subscriber_count,
        other => panic!("expected Publish payload, got {:?}", other),
    };
    assert_eq!(count, 1, "expected 1 subscriber");

    // Subscriber reads the pushed event (request_id=0, Payload::PubSubEvent).
    let mut prefix = [0u8; 4];
    sub.stream.read_exact(&mut prefix).expect("read event prefix");
    let len = u32::from_be_bytes(prefix) as usize;
    let mut body = vec![0u8; len];
    sub.stream.read_exact(&mut body).expect("read event body");
    let event_resp = pb::Response::decode(body.as_slice()).expect("decode event");
    assert_eq!(event_resp.request_id, 0, "push events have request_id=0");
    let event = match event_resp.payload {
        Some(pb::response::Payload::PubsubEvent(ref e)) => e.clone(),
        other => panic!("expected PubSubEvent payload, got {:?}", other),
    };
    assert_eq!(event.topic, "sensors");
    assert_eq!(event.payload, vec![10, 20, 30]);

    // Unsubscribe
    let unsub_resp = sub.round_trip(pb::Request {
        request_id: 3,
        op: Some(pb::request::Op::Unsubscribe(pb::UnsubscribeRequest {
            topic: "sensors".into(),
            subscription_id: sub_id,
        })),
    });
    let unsub_status = unsub_resp.status.expect("unsubscribe status present");
    assert_eq!(unsub_status.code, 0, "unsubscribe should succeed");

    // Publish again — subscriber should NOT receive event (count=0 for this
    // publisher's view, but the subscriber is already unsubscribed).
    let pub2_resp = pub_.round_trip(pb::Request {
        request_id: 4,
        op: Some(pb::request::Op::Publish(pb::PublishRequest {
            topic: "sensors".into(),
            payload: vec![99],
        })),
    });
    let pub2_status = pub2_resp.status.expect("publish status present");
    assert_eq!(pub2_status.code, 0);
    let count2 = match pub2_resp.payload {
        Some(pb::response::Payload::Publish(ref p)) => p.subscriber_count,
        other => panic!("expected Publish payload, got {:?}", other),
    };
    assert_eq!(count2, 0, "expected 0 subscribers after unsubscribe");
}