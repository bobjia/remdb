# remdb-server

A lean TCP query server for [remdb](/workspace) over a protobuf protocol, with
a full-stack **zero-copy** result path: query results flow from remdb's
contiguous `CompactResultSet.raw_data` straight into the protobuf `bytes` field
(no per-row allocation on the serialize/wire path).

`std`-only. It depends on `remdb` (`std` feature) and serves over length-prefixed
protobuf frames on TCP, spawning one thread per connection.

## Run

```bash
cargo run -p remdb-server -- 127.0.0.1:9999
```

Arguments: the first positional argument is the bind address (defaults to
`127.0.0.1:9999`).

## Wire format

Requests and responses are length-prefixed protobuf frames:

```
<u32 BE length> <protobuf Request>   -->  request
                                           |       |
                                           v       v
<u32 BE length> <protobuf Response>  <--  response
```

Decoding/encoding is handled by `src/protocol.rs` (max frame size 64 MiB).

## Supported request ops

- **query** — raw SQL `SELECT` (zero-copy result path).
- **ddl** — raw SQL `CREATE`/`ALTER`/`DROP` statements.
- **crud** — structured insert/update/delete translated to SQL.
- **schema** — `DESCRIBE <table>` and `SHOW TABLES` through the SQL engine.
- **ping** — health check returning the server version.

See `proto/remdb.proto` for the exact message layout.

## Design

See the design document: [`docs/designs/2026-08-21-remdb-server-design.md`](/workspace/docs/designs/2026-08-21-remdb-server-design.md).

## Benchmarks

```bash
cargo bench -p remdb-server   # benchmarks handle_request (no network)
```