//! Convert `CompactResultSet` into the protobuf `QueryResponse`, copying its
//! contiguous `raw_data` straight into the response `bytes` field.

use crate::pb::column_info::Type;
use crate::pb::QueryResponse;
use remdb::DataType;

/// Map a remdb `DataType` to the protobuf column type enum.
pub fn map_type(dt: DataType) -> Type {
    match dt {
        DataType::UInt8 => Type::Uint8,
        DataType::UInt16 => Type::Uint16,
        DataType::UInt32 => Type::Uint32,
        DataType::UInt64 => Type::Uint64,
        DataType::Int8 => Type::Int8,
        DataType::Int16 => Type::Int16,
        DataType::Int32 => Type::Int32,
        DataType::Int64 => Type::Int64,
        DataType::Float32 => Type::Float32,
        DataType::Float64 => Type::Float64,
        DataType::Bool => Type::Bool,
        DataType::Timestamp | DataType::TimestampTZ => Type::Timestamp,
        DataType::String => Type::String,
        DataType::Interval => Type::Interval,
    }
}

/// Build a `QueryResponse` from a zero-copy `CompactResultSet`.
/// This performs a single `extend_from_slice` of the whole `raw_data`; no
/// per-row `TypedValue` / `Vec` is allocated on the serialization path.
pub fn build_query_response(set: &remdb::sql::CompactResultSet) -> Option<QueryResponse> {
    let columns = set
        .columns
        .iter()
        .map(|c| crate::pb::ColumnInfo {
            name: c.name.clone(),
            r#type: map_type(c.data_type) as i32,
            offset: c.offset as u32,
        })
        .collect();

    let mut raw = Vec::with_capacity(set.raw_data.len());
    raw.extend_from_slice(&set.raw_data);

    Some(QueryResponse {
        columns,
        raw_data: raw,
        record_size: set.record_size as u32,
        record_count: set.record_count as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pb::column_info::Type as P;
    use remdb::DataType;

    fn col(name: &str, data_type: DataType, offset: usize, size: usize) -> remdb::sql::ColumnInfo {
        remdb::sql::ColumnInfo { name: name.to_string(), offset, data_type, size }
    }

    #[test]
    fn maps_all_data_types() {
        assert_eq!(map_type(DataType::UInt8), P::Uint8);
        assert_eq!(map_type(DataType::UInt16), P::Uint16);
        assert_eq!(map_type(DataType::UInt32), P::Uint32);
        assert_eq!(map_type(DataType::UInt64), P::Uint64);
        assert_eq!(map_type(DataType::Int8), P::Int8);
        assert_eq!(map_type(DataType::Int16), P::Int16);
        assert_eq!(map_type(DataType::Int32), P::Int32);
        assert_eq!(map_type(DataType::Int64), P::Int64);
        assert_eq!(map_type(DataType::Float32), P::Float32);
        assert_eq!(map_type(DataType::Float64), P::Float64);
        assert_eq!(map_type(DataType::Bool), P::Bool);
        assert_eq!(map_type(DataType::Timestamp), P::Timestamp);
        assert_eq!(map_type(DataType::TimestampTZ), P::Timestamp);
        assert_eq!(map_type(DataType::String), P::String);
        assert_eq!(map_type(DataType::Interval), P::Interval);
    }

    #[test]
    fn serializes_raw_data_without_per_row_alloc() {
        let columns = vec![col("a", DataType::UInt32, 0, 4)];
        let mut set = remdb::sql::CompactResultSet::new(columns, 4);
        set.add_record(&[1u8, 0, 0, 0]).expect("add");
        set.add_record(&[2u8, 0, 0, 0]).expect("add");

        let qr = build_query_response(&set);
        let qr = qr.expect("built");
        assert_eq!(qr.record_count, 2);
        assert_eq!(qr.record_size, 4);
        assert_eq!(qr.raw_data.len(), 8);
        assert_eq!(qr.columns.len(), 1);
        assert_eq!(qr.columns[0].name, "a");
        assert_eq!(qr.columns[0].r#type, P::Uint32 as i32);
        assert_eq!(qr.columns[0].offset, 0);
        assert_eq!(qr.raw_data[0], 1u8);
        assert_eq!(qr.raw_data[4], 2u8);
    }
}