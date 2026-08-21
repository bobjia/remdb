//! Zero-copy query path benchmarks.
//!
//! Benchmarks comparing the old path (ResultSet + TypedValue) with the new
//! zero-copy path (RawRecordView + CompactResultSet).

extern crate alloc;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use remdb::table::*;
use remdb::types::*;
use remdb::platform::*;
use remdb::RawRecordView;
use remdb::CompactResultSet;
use remdb::sql::ColumnInfo;
use alloc::sync::Arc;

// Test platform implementation
struct TestPlatform;

impl Platform for TestPlatform {
    fn get_timestamp(&self) -> u64 { 0 }
    fn get_timestamp_us(&self) -> u64 { 0 }
    fn memcpy(&self, dest: &mut [u8], src: &[u8]) {
        let len = core::cmp::min(dest.len(), src.len());
        dest[..len].copy_from_slice(&src[..len]);
    }
    fn memset(&self, dest: &mut [u8], value: u8) {
        dest.fill(value);
    }
    fn delay_ms(&self, _ms: u32) {}
    fn delay_us(&self, _us: u32) {}
    fn file_open(&self, _path: &str, _mode: FileMode) -> FileResult<FileHandle> { Ok(0) }
    fn file_close(&self, _handle: FileHandle) -> FileResult<()> { Ok(()) }
    fn file_write(&self, _handle: FileHandle, _buf: &[u8]) -> FileResult<usize> { Ok(0) }
    fn file_read(&self, _handle: FileHandle, _buf: &mut [u8]) -> FileResult<usize> { Ok(0) }
    fn file_seek(&self, _handle: FileHandle, _offset: i64, _whence: SeekWhence) -> FileResult<u64> { Ok(0) }
    fn file_remove(&self, _path: &str) -> FileResult<()> { Ok(()) }
    fn file_size(&self, _path: &str) -> FileResult<usize> { Ok(0) }
    fn crc32(&self, _data: &[u8]) -> u32 { 0 }
}

static TEST_PLATFORM: TestPlatform = TestPlatform;

// Table definition for benchmarks
static BENCH_TABLE_DEF: TableDef = TableDef {
    id: 0,
    name: "bench_table",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::UInt32,
            size: 4,
            offset: 0,
            not_null: true,
            primary_key: true,
            unique: true,
            auto_increment: true,
            default_value: None,
        },
        FieldDef {
            name: "value",
            data_type: DataType::Float64,
            size: 8,
            offset: 4,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "name",
            data_type: DataType::String,
            size: 64,
            offset: 12,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "active",
            data_type: DataType::Bool,
            size: 1,
            offset: 76,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
    ],
    primary_key: 0,
    secondary_index: None,
    secondary_index_type: IndexType::SortedArray,
    record_size: 77,
    max_records: 1000,
};

/// Create a table with N records for benchmarking.
fn create_populated_table(n: usize) -> MemoryTable {
    let mut table = MemoryTable::new(Arc::new(BENCH_TABLE_DEF)).unwrap();
    for i in 0..n {
        let mut record = vec![0u8; 77];
        record[0..4].copy_from_slice(&(i as u32).to_le_bytes());
        record[4..12].copy_from_slice(&(i as f64 * 1.5).to_le_bytes());
        let name = alloc::format!("record_{}", i);
        let name_bytes = name.as_bytes();
        let copy_len = core::cmp::min(name_bytes.len(), 64);
        record[12..12 + copy_len].copy_from_slice(&name_bytes[..copy_len]);
        record[76] = if i % 2 == 0 { 1 } else { 0 };
        table.insert(&record).unwrap();
    }
    table
}

/// Benchmark: RawRecordView field access (zero-copy) vs table.get_field (Value allocation).
fn bench_raw_record_view_access(c: &mut Criterion) {
    init_platform(&TEST_PLATFORM);

    const MEMORY_SIZE: usize = 1024 * 1024;
    // Initialize global memory allocator
    let mut memory = vec![0u8; MEMORY_SIZE];
    remdb::memory::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("raw_record_view");

    group.bench_function("get_field_old_path", |b| {
        b.iter(|| {
            remdb::memory::reset_global_allocator().unwrap();
            let table = create_populated_table(100);
            for i in 0..100 {
                let record_slice = table.get_record_slice(i);
                // Old path: table.get_field creates a Value enum
                let _val = black_box(table.get_field(record_slice, 0).unwrap());
            }
        })
    });

    group.bench_function("raw_record_view_zero_copy", |b| {
        b.iter(|| {
            remdb::memory::reset_global_allocator().unwrap();
            let table = create_populated_table(100);
            for i in 0..100 {
                let record_slice = table.get_record_slice(i);
                let view = RawRecordView::new(record_slice, &table.def);
                let _val = black_box(view.read_u32(0).unwrap());
            }
        })
    });

    group.finish();
}

/// Benchmark: CompactResultSet creation and typed access.
fn bench_compact_result_set(c: &mut Criterion) {
    init_platform(&TEST_PLATFORM);

    const MEMORY_SIZE: usize = 1024 * 1024;
    // Initialize global memory allocator
    let mut memory = vec![0u8; MEMORY_SIZE];
    remdb::memory::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("compact_result_set");

    // Create a CompactResultSet once for the benchmark
    let columns = vec![
        ColumnInfo { name: "id".to_string(), offset: 0, data_type: DataType::UInt32, size: 4 },
        ColumnInfo { name: "value".to_string(), offset: 4, data_type: DataType::Float64, size: 8 },
        ColumnInfo { name: "name".to_string(), offset: 12, data_type: DataType::String, size: 64 },
        ColumnInfo { name: "active".to_string(), offset: 76, data_type: DataType::Bool, size: 1 },
    ];

    group.bench_function("add_records", |b| {
        b.iter(|| {
            let mut rs = CompactResultSet::new(columns.clone(), 77);
            for i in 0..100 {
                let mut record = vec![0u8; 77];
                record[0..4].copy_from_slice(&(i as u32).to_le_bytes());
                record[4..12].copy_from_slice(&(i as f64 * 1.5).to_le_bytes());
                record[76] = if i % 2 == 0 { 1 } else { 0 };
                black_box(rs.add_record(&record).unwrap());
            }
        })
    });

    group.bench_function("typed_access_all_rows", |b| {
        b.iter(|| {
            let mut rs = CompactResultSet::new(columns.clone(), 77);
            for i in 0..100 {
                let mut record = vec![0u8; 77];
                record[0..4].copy_from_slice(&(i as u32).to_le_bytes());
                record[4..12].copy_from_slice(&(i as f64 * 1.5).to_le_bytes());
                record[76] = if i % 2 == 0 { 1 } else { 0 };
                rs.add_record(&record).unwrap();
            }
            // Access every field in every row using typed accessors
            for row in 0..100 {
                let _id = black_box(rs.get_field_u32(row, 0).unwrap());
                let _val = black_box(rs.get_field_f64(row, 1).unwrap());
                let _active = black_box(rs.get_field_bool(row, 3).unwrap());
            }
        })
    });

    group.bench_function("get_row_typed_all_rows", |b| {
        b.iter(|| {
            let mut rs = CompactResultSet::new(columns.clone(), 77);
            for i in 0..100 {
                let mut record = vec![0u8; 77];
                record[0..4].copy_from_slice(&(i as u32).to_le_bytes());
                record[4..12].copy_from_slice(&(i as f64 * 1.5).to_le_bytes());
                record[76] = if i % 2 == 0 { 1 } else { 0 };
                rs.add_record(&record).unwrap();
            }
            // Access every row using the backwards-compat get_row (Vec<TypedValue>)
            for row in 0..100 {
                let _row = black_box(rs.get_row(row).unwrap());
            }
        })
    });

    group.finish();
}

criterion_group!(
    zero_copy_benches,
    bench_raw_record_view_access,
    bench_compact_result_set,
);
criterion_main!(zero_copy_benches);