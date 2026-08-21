# Fast Zero-Copy remdb-server (protobuf)

**Date:** 2026-08-21
**Status:** Draft

## Purpose

Provide a lean TCP query server for `remdb` using a protobuf wire protocol,
with a full-stack zero-copy data path. The purpose is to let external clients
query the embedded in-memory database over a binary protocol without dragging
the core library off its lean / `no_std` philosophy.

## Scope

- New isolated workspace crate `crates/remdb-server` (std-only).
- TCP server, thread-per-connection, serialized DB access via the existing
  `Mutex<RemDb>` model.
- protobuf protocol implemented with `prost` / `prost-build`.
- Zero-copy at every stage: request parse, query execution, raw result to wire,
  and buffer pooling.
- Core `remdb` stays `no_std`-clean; the server crate never participates in the
  core `--no-default-features` build.

Out of scope: async runtime (tokio), connection pooling daemon, JDBC driver,
full DDL statement grammar in the server (DDL delegates to the SQL parser).

## Architecture

```
crates/remdb-server/
├── Cargo.toml
├── build.rs            # prost-build → compiles proto/remdb.proto
├── proto/remdb.proto
├── src/
│   ├── lib.rs          # re-exports: Protocol, Server, Frame
│   ├── protocol.rs     # Request/Response envelope + length-prefix framing codec
│   ├── buffer.rs       # BufferPool (reusable socket/protobuf buffers)
│   ├── serialize.rs    # CompactResultSet raw_data → QueryResponse (zero-copy)
│   ├── server.rs       # accept loop + thread-per-connection handler
│   └── main.rs         # thin CLI binary (bind addr, port)
└── tests/
    └── client_test.rs  # integration: real client ↔ server
```

**Concurrency:** `TcpListener::accept()` loop → `thread::spawn` per connection.
Each handler holds a clone of `Arc<Mutex<RemDb>>`; all DB access remains
serialized, preserving remdb's single-writer concurrency assumptions.

**Zero-copy targets:**
- *Query-exec:* reuse `CompactResultSet` / `RawRecordView` so rows are never
  materialized as `Vec<Vec<TypedValue>>` before serialization.
- *Raw result → wire:* `serialize.rs` fills `QueryResponse.raw_data` from
  `CompactResultSet.raw_data` with a single `extend_from_slice`; no per-row
  intermediate buffers.
- *Buffer pooling:* `buffer.rs` recycles length-prefix read buffers across
  messages within a connection.
- *Zero-copy request parse:* SQL string is borrowed from the protobuf-decoded
  bytes field (no extra copy).

## Wire Format

Framing: `<u32 BE length><protobuf bytes>` over TCP. The reader reads exactly
one frame into a pooled buffer, giving a clean message boundary for reuse.

Common envelope on every message:

```protobuf
syntax = "proto3";
package remdb;

message Request {
  uint64 request_id = 1;
  oneof op {
    QueryRequest   query   = 2;   // raw SQL passthrough
    DdlRequest     ddl     = 3;   // brief DDL SQL (delegates to SQL parser)
    CrudRequest    crud    = 4;   // structured INSERT/UPDATE/DELETE
    SchemaRequest  schema  = 5;   // describe/list tables
    PingRequest    ping    = 6;
  }
}

message Response {
  uint64 request_id = 1;
  Status status = 2;
  oneof payload {
    QueryResponse   query   = 3;
    DdlResponse     ddl     = 4;
    CrudResponse    crud    = 5;
    SchemaResponse  schema  = 6;
    PingResponse    ping    = 7;
    MetricsResponse metrics = 8;
  }
}

message Status {
  enum Code { OK = 0; ERROR = 1; NOT_FOUND = 2; }
  Code code = 1;
  string message = 2;
}
```

**Query result (rows as raw columnar bytes, not nested messages):**

```protobuf
message QueryResponse {
  repeated ColumnInfo columns = 3;
  bytes   raw_data = 9;          // CompactResultSet.raw_data, row-major
  uint32  record_size = 10;      // bytes per row
  uint32  record_count = 11;     // row count
}
message ColumnInfo {
  string name = 1;
  enum Type { UINT8; UINT16; UINT32; UINT64;
              INT8; INT16; INT32; INT64;
              FLOAT32; FLOAT64; BOOL; TIMESTAMP; STRING; INTERVAL; }
  Type type = 2;
  uint32 offset = 3;             // field byte offset within row
}
```

This mirrors `CompactResultSet`'s layout exactly so `serialize.rs` fills
`raw_data` with one `extend_from_slice` and clients decode rows with the same
offset math.

## Structured Ops & Types

Generic `Value` shared by CRUD/Ddl:

```protobuf
message Value {
  oneof v {
    uint64   v_uint   = 1;
    int64    v_int    = 2;
    double   v_double = 3;
    bool     v_bool   = 4;
    string   v_str    = 5;
    bytes    v_bytes  = 6;   // covers String raw / Interval / Timestamp raw
  }
}
```

Structured messages:

```protobuf
message CrudRequest { string table = 1; oneof op { Insert insert = 2;
                    Update update = 3; Delete delete = 4; } }
message Insert { repeated Value values = 1; }
message Update { repeated string cols = 1; repeated Value values = 2;
                 repeated Condition where = 3; }
message Delete { repeated Condition where = 1; }
message Condition { string column = 1; string op = 2; Value value = 3; }

message SchemaRequest { oneof op { string describe = 1; bool list = 2; } }
message SchemaResponse { repeated TableSchema tables = 1; }
message TableSchema { string name = 1; repeated FieldSchema fields = 2; }
```

DDL is a thin path (`DdlRequest { string sql = 1; }`) translated through remdb's
`DdlExecutor`. Statement grammar stays in one place (the SQL parser) rather than
being duplicated in the server.

## Data Flow

1. Read one frame → length-prefixed pooled buffer.
2. Zero-copy decode `Request` (SQL string borrowed from buffer).
3. Acquire `Mutex<RemDb>`, route to `sql_query` / DDL / CRUD / schema.
4. Query path returns `CompactResultSet` → `serialize.rs` fills
   `QueryResponse.raw_data` with one `extend_from_slice`.
5. Encode `Response` into a pooled buffer, write frame.

## Error Handling

All errors become `Response.status` (`ERROR` / `NOT_FOUND`) with a message;
`request_id` is preserved for correlation. Malformed or oversized frames produce
an error response or a connection close. The framing codec loops until a full
frame is read, handling partial reads.

## Testing Plan

**Unit tests (in `remdb-server`):**
- Framing codec: round-trip for all op types; partial reads; multi-message
  coalescing; oversized-frame rejection.
- Zero-copy serialize: every datatype round-trips through `raw_data`; multiple
  rows; empty result; STRING / TIMESTAMP / Interval fields.
- Value ↔ remdb conversion: every protobuf `Value` variant maps to the correct
  field type and round-trips.
- Buffer pool: recycle correctness under concurrent threads.

**Integration tests (`tests/client_test.rs`, real TCP):**
- DDL CREATE TABLE, CRUD INSERT, query SELECT; verify `raw_data` decodes to
  expected rows.
- Errors: malformed request → `status.error`; unknown table → `NOT_FOUND`.
- Zero-copy verification: assert no per-row allocations inside `serialize.rs`.
- Concurrency: N clients × M queries; assert no corruption and correct
  `request_id` correlation.

**Benchmark (`benches/zero_copy_server.rs`, optional):** queries/sec +
allocations/query at 1/10/100/1000 rows, reusing existing criterion setup.

**Guardrail:** `remdb-server` is std-only and excluded from the core library's
`--no-default-features` check.

## Open Questions

- Should the address/port config be env-var / CLI only, or integrate with
  `DbConfig`? (Provisional: CLI args; keep config out of the core.)
- Whether to publish `remdb-server` as a binary crate or `lib + bin`.