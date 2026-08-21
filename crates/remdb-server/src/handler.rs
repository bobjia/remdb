//! Protocol dispatch: a decoded `Request` → one `Response`.

use crate::pb::response::Payload;
use crate::pb::status::Code;
use crate::serialize::build_query_response;
use remdb::RemDb;

/// A typed database handle shared across connection threads.
pub type SharedDb = std::sync::Arc<std::sync::Mutex<RemDb>>;

/// Execute a decoded protobuf `Request` against the shared DB and produce a
/// `Response` with the same `request_id`.
pub fn handle_request(db: &SharedDb, request: &crate::pb::Request) -> crate::pb::Response {
    let request_id = request.request_id;
    let build = |status: crate::pb::Status, payload: Option<Payload>| crate::pb::Response {
        request_id,
        status: Some(status),
        payload,
    };

    let op = match &request.op {
        Some(op) => op.clone(),
        None => return build(err_status("missing operation"), None),
    };

    let mut db = db
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    match op {
        crate::pb::request::Op::Ping(_) => {
            let ping = crate::pb::PingResponse { server_version: "0.1.0".to_string() };
            build(ok_status(), Some(Payload::Ping(ping)))
        }
        crate::pb::request::Op::Query(q) => {
            match remdb::sql::parse_sql_query(&q.sql) {
                Ok(parsed) => match remdb::sql::execute_query_raw(&mut db, &parsed) {
                    Ok(set) => match build_query_response(&set) {
                        Some(qr) => build(ok_status(), Some(Payload::Query(qr))),
                        None => build(err_status("failed to build response"), None),
                    },
                    Err(e) => build(err_status(&format!("query error: {:?}", e)), None),
                },
                Err(e) => build(err_status(&format!("parse error: {:?}", e)), None),
            }
        }
        crate::pb::request::Op::Ddl(d) => {
            match run_sql(&mut db, &d.sql) {
                Ok(_) => {
                    let ddl = crate::pb::DdlResponse { message: "ok".to_string() };
                    build(ok_status(), Some(Payload::Ddl(ddl)))
                }
                Err(msg) => build(err_status(&msg), None),
            }
        }
        crate::pb::request::Op::Crud(c) => {
            let sql = crud_to_sql(&c);
            match run_sql(&mut db, &sql) {
                Ok(affected) => {
                    let crud = crate::pb::CrudResponse { affected: affected as u32 };
                    build(ok_status(), Some(Payload::Crud(crud)))
                }
                Err(msg) => build(err_status(&msg), None),
            }
        }
        crate::pb::request::Op::Schema(s) => {
            // Route DESCRIBE/list through the SQL engine.
            let sql = match s.op {
                Some(crate::pb::schema_request::Op::Describe(tbl)) => {
                    format!("DESCRIBE {}", tbl)
                }
                Some(crate::pb::schema_request::Op::List(_)) => "SHOW TABLES".to_string(),
                None => "SHOW TABLES".to_string(),
            };
            match remdb::sql::parse_sql_query(&sql) {
                Ok(parsed) => match remdb::sql::execute_query_raw(&mut db, &parsed) {
                    Ok(set) => match build_query_response(&set) {
                        Some(qr) => build(ok_status(), Some(Payload::Query(qr))),
                        None => build(err_status("failed to build schema response"), None),
                    },
                    Err(e) => build(err_status(&format!("schema error: {:?}", e)), None),
                },
                Err(e) => build(err_status(&format!("schema parse error: {:?}", e)), None),
            }
        }
    }
}

fn ok_status() -> crate::pb::Status {
    crate::pb::Status { code: Code::Ok as i32, message: String::new() }
}

fn err_status(message: &str) -> crate::pb::Status {
    crate::pb::Status { code: Code::Error as i32, message: message.to_string() }
}

/// Run a statement; returns affected row count. Uses the zero-copy
/// `execute_query_raw` which falls back to the legacy path for DDL/CRUD.
fn run_sql(db: &mut RemDb, sql: &str) -> Result<usize, String> {
    let parsed = remdb::sql::parse_sql_query(sql).map_err(|e| format!("parse error: {:?}", e))?;
    let set = remdb::sql::execute_query_raw(db, &parsed)
        .map_err(|e| format!("exec error: {:?}", e))?;
    Ok(set.record_count)
}

/// Translate a structured CRUD request into a SQL statement.
/// String values are single-quoted; numeric/bool values are bare literals.
fn crud_to_sql(c: &crate::pb::CrudRequest) -> String {
    match &c.op {
        Some(crate::pb::crud_request::Op::Insert(ins)) => {
            let cols: Vec<String> = ins
                .values
                .iter()
                .enumerate()
                .map(|(i, _)| format!("c{}", i))
                .collect();
            let vals: Vec<String> = ins.values.iter().map(value_to_literal).collect();
            format!(
                "INSERT INTO {} ({}) VALUES ({})",
                c.table,
                cols.join(", "),
                vals.join(", ")
            )
        }
        Some(crate::pb::crud_request::Op::Update(up)) => {
            let sets: Vec<String> = up
                .cols
                .iter()
                .zip(up.values.iter())
                .map(|(col, val)| format!("{} = {}", col, value_to_literal(val)))
                .collect();
            let where_sql = conditions_to_sql(&up.r#where);
            let suffix = if where_sql.is_empty() {
                String::new()
            } else {
                format!(" WHERE {}", where_sql)
            };
            format!("UPDATE {} SET {}{}", c.table, sets.join(", "), suffix)
        }
        Some(crate::pb::crud_request::Op::Delete(del)) => {
            let where_sql = conditions_to_sql(&del.r#where);
            let suffix = if where_sql.is_empty() {
                String::new()
            } else {
                format!(" WHERE {}", where_sql)
            };
            format!("DELETE FROM {}{}", c.table, suffix)
        }
        None => format!("SELECT 1 FROM {} WHERE 1=0", c.table),
    }
}

fn conditions_to_sql(conds: &[crate::pb::Condition]) -> String {
    conds
        .iter()
        .map(|c| {
            let lit = match c.value.as_ref() {
                Some(v) => value_to_literal(v),
                None => "NULL".to_string(),
            };
            format!("{} {} {}", c.column, c.op, lit)
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn value_to_literal(v: &crate::pb::Value) -> String {
    match &v.v {
        Some(crate::pb::value::V::VUint(n)) => format!("{}", n),
        Some(crate::pb::value::V::VInt(n)) => format!("{}", n),
        Some(crate::pb::value::V::VDouble(d)) => format!("{}", d),
        Some(crate::pb::value::V::VBool(b)) => format!("{}", b),
        Some(crate::pb::value::V::VStr(s)) => {
            let escaped = s.replace('\'', "''");
            format!("'{}'", escaped)
        }
        Some(crate::pb::value::V::VBytes(b)) => {
            // interpret utf-8 bytes as a quoted string; fall back to empty
            match std::str::from_utf8(b.as_slice()) {
                Ok(s) => value_to_literal(&crate::pb::Value {
                    v: Some(crate::pb::value::V::VStr(s.to_string())),
                }),
                Err(_) => "''".to_string(),
            }
        }
        None => "NULL".to_string(),
    }
}