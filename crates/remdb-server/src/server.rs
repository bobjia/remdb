//! TCP server: accept loop, per-connection threads, and frame dispatch.

use crate::handler::{handle_request, SharedDb};
use crate::pb::Request;
use crate::protocol::{encode_frame, FrameDecoder};
use prost::Message as ProstMessage;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};

const READ_BUF_SIZE: usize = 64 * 1024;

/// Accept connections on `listener`, spawning one thread per connection.
pub fn serve(listener: TcpListener, db: SharedDb) -> io::Result<()> {
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
        std::thread::spawn(move || {
            if let Err(e) = handle_connection(stream, db) {
                eprintln!("connection error: {e}");
            }
        });
    }
    Ok(())
}

/// Read frames from a single connection and dispatch them until EOF/error.
fn handle_connection(mut stream: TcpStream, db: SharedDb) -> io::Result<()> {
    let mut decoder = FrameDecoder::new();
    let mut read_buf = vec![0u8; READ_BUF_SIZE];
    let mut frame_buf = Vec::new();

    loop {
        let n = stream.read(&mut read_buf)?;
        if n == 0 {
            return Ok(()); // EOF
        }
        let mut remaining = read_buf.get(..n).unwrap_or(&[]);
        while let Some((payload, rest)) = decoder.next(remaining)? {
            process_frame(&mut stream, &db, payload.as_ref(), &mut frame_buf)?;
            remaining = rest;
        }
    }
}

/// Decode one request frame, run it against the shared DB, and write the
/// response frame back to the client.
fn process_frame(
    stream: &mut TcpStream,
    db: &SharedDb,
    payload: &[u8],
    frame_buf: &mut Vec<u8>,
) -> io::Result<()> {
    let request = Request::decode(payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let response = handle_request(db, &request);
    let encoded = response.encode_to_vec();
    frame_buf.clear();
    frame_buf.extend_from_slice(&encode_frame(&encoded));
    stream.write_all(frame_buf)?;
    stream.flush()
}