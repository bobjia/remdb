//! TCP server: accept loop, per-connection threads, frame dispatch, and
//! pub/sub push thread.

use crate::handler::{handle_request, SharedDb};
use crate::pb::Request;
use crate::protocol::{encode_frame, FrameDecoder};
use crate::pubsub::{Event, PubSubManager};
use prost::Message as ProstMessage;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

const READ_BUF_SIZE: usize = 64 * 1024;

/// Accept connections on `listener`, spawning one thread per connection.
pub fn serve(
    listener: TcpListener,
    db: SharedDb,
    pubsub: Arc<PubSubManager>,
) -> io::Result<()> {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let db = db.clone();
        let pubsub = pubsub.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle_connection(stream, db, pubsub) {
                eprintln!("connection error: {e}");
            }
        });
    }
    Ok(())
}

/// Read frames from a single connection, dispatch them, and manage the
/// pub/sub push thread.
fn handle_connection(
    mut stream: TcpStream,
    db: SharedDb,
    pubsub: Arc<PubSubManager>,
) -> io::Result<()> {
    // Channel for pub/sub events pushed to this connection.
    let (event_tx, event_rx) = mpsc::channel::<Event>();
    // Write lock so the response thread and push thread don't interleave.
    let write_lock = Arc::new(Mutex::new(()));
    let mut stream_clone = stream.try_clone()?;
    let push_running = Arc::new(AtomicBool::new(true));

    // Spawn a push thread that forwards pub/sub events to the client.
    let push_handle = {
        let write_lock = write_lock.clone();
        let push_running = push_running.clone();
        std::thread::spawn(move || {
            while let Ok((topic, payload)) = event_rx.recv() {
                if !push_running.load(Ordering::Relaxed) {
                    break;
                }
                let event = crate::pb::PubSubEvent {
                    topic,
                    payload,
                };
                let response = crate::pb::Response {
                    request_id: 0,
                    status: Some(crate::pb::Status {
                        code: 0,
                        message: String::new(),
                    }),
                    payload: Some(crate::pb::response::Payload::PubsubEvent(event)),
                };
                let encoded = response.encode_to_vec();
                let frame = encode_frame(&encoded);
                let _lock = write_lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if stream_clone.write_all(&frame).is_err() {
                    break; // connection closed
                }
            }
        })
    };

    // Track active subscriptions for cleanup on disconnect.
    let mut subscriptions: Vec<(String, u64)> = Vec::new();

    let mut decoder = FrameDecoder::new();
    let mut read_buf = vec![0u8; READ_BUF_SIZE];
    let mut frame_buf = Vec::new();

    let result = loop {
        let n = match stream.read(&mut read_buf) {
            Ok(0) => break Ok(()), // EOF
            Ok(n) => n,
            Err(e) => break Err(e),
        };
        let mut remaining = read_buf.get(..n).unwrap_or(&[]);
        while let Some((payload, rest)) = decoder.next(remaining)? {
            // Process the request frame.
            match process_frame(&mut stream, &db, &pubsub, &event_tx, &*write_lock, payload.as_ref(), &mut frame_buf, &mut subscriptions) {
                Ok(()) => {}
                Err(e) => {
                    // Clean up subscriptions on error.
                    pubsub.unsubscribe_all(&subscriptions);
                    push_running.store(false, Ordering::Relaxed);
                    return Err(e);
                }
            }
            remaining = rest;
        }
    };

    // Clean up subscriptions on graceful disconnect.
    pubsub.unsubscribe_all(&subscriptions);
    push_running.store(false, Ordering::Relaxed);
    let _ = push_handle.join();
    result
}

/// Decode one request frame, run it against the shared DB, and write the
/// response frame back to the client.
fn process_frame(
    stream: &mut TcpStream,
    db: &SharedDb,
    pubsub: &PubSubManager,
    event_tx: &mpsc::Sender<Event>,
    write_lock: &Mutex<()>,
    payload: &[u8],
    frame_buf: &mut Vec<u8>,
    subscriptions: &mut Vec<(String, u64)>,
) -> io::Result<()> {
    let request = Request::decode(payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let response = handle_request(db, pubsub, event_tx, &request);

    // If this was a subscribe, record the subscription id for cleanup.
    if let Some(crate::pb::response::Payload::Subscribe(ref sub_resp)) = response.payload {
        if let Some(crate::pb::request::Op::Subscribe(ref s)) = request.op {
            subscriptions.push((s.topic.clone(), sub_resp.subscription_id));
        }
    }

    // If this was an unsubscribe, remove the subscription from our tracking.
    if let Some(crate::pb::request::Op::Unsubscribe(ref u)) = request.op {
        subscriptions.retain(|(t, id)| t != &u.topic || *id != u.subscription_id);
    }

    let encoded = response.encode_to_vec();
    frame_buf.clear();
    frame_buf.extend_from_slice(&encode_frame(&encoded));
    let _lock = write_lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    stream.write_all(frame_buf)?;
    stream.flush()
}