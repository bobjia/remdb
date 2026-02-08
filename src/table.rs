use crate::defer;
use crate::platform::{memcpy, memset};
use crate::types::{DataType, RecordHeader, RecordStatus, RemDbError, Result, TableDef, Value};
use core::ptr::NonNull;

// 引入alloc模块
extern crate alloc;
use alloc::vec::Vec;

/// 内存表
pub struct MemoryTable {
    /// 表定义
    pub def: alloc::sync::Arc<TableDef>,
    /// 表数据起始地址
    pub data_start: NonNull<u8>,
    /// 记录状态数组
    pub status_array: NonNull<RecordHeader>,
    /// 当前记录数
    pub record_count: usize,
    /// 自旋锁
    pub lock: u32,
    /// 记录大小（运行时计算）
    pub record_size: usize,
    /// 空闲记录槽栈（优化插入性能）
    pub free_slots: NonNull<usize>,
    /// 空闲记录槽数量
    pub free_slot_count: usize,
    /// 是否处于低功耗模式
    pub low_power_mode: bool,
    /// 低功耗模式下的最大记录数
    pub low_power_max_records: Option<usize>,
    /// 表快照版本号
    pub snapshot_version: u32,
    /// 最大主键值（用于优化自增ID生成）
    pub max_pk: u64,
}

/// 记录只读引用（零拷贝视图）
///
/// # 生命周期与稳定性
/// - 记录地址在此引用生命周期内保持稳定
/// - 不允许在外部直接修改内部内存
/// - 并发删除/覆盖可能导致引用失效，请在事务或安全借用范围内使用
#[derive(Copy, Clone)]
pub struct RecordRef<'a> {
    table: &'a MemoryTable,
    id: usize,
    record_ptr: *const u8,
}

impl<'a> RecordRef<'a> {
    /// 记录ID
    pub fn id(&self) -> usize {
        self.id
    }

    /// 表定义
    pub fn table_def(&self) -> &'a TableDef {
        self.table.def.as_ref()
    }

    fn field_def(&self, col: usize) -> Result<&'a crate::types::FieldDef> {
        self.table
            .def
            .fields
            .get(col)
            .ok_or(RemDbError::FieldNotFound)
    }

    fn field_ptr(&self, field: &crate::types::FieldDef) -> *const u8 {
        unsafe { self.record_ptr.add(field.offset) }
    }

    /// 按列索引读取u8
    pub fn get_u8(&self, col: usize) -> Result<u8> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::UInt8 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const u8) })
    }

    /// 按列索引读取u16
    pub fn get_u16(&self, col: usize) -> Result<u16> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::UInt16 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const u16) })
    }

    /// 按列索引读取u32
    pub fn get_u32(&self, col: usize) -> Result<u32> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::UInt32 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const u32) })
    }

    /// 按列索引读取u64
    pub fn get_u64(&self, col: usize) -> Result<u64> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::UInt64 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const u64) })
    }

    /// 按列索引读取i8
    pub fn get_i8(&self, col: usize) -> Result<i8> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Int8 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const i8) })
    }

    /// 按列索引读取i16
    pub fn get_i16(&self, col: usize) -> Result<i16> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Int16 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const i16) })
    }

    /// 按列索引读取i32
    pub fn get_i32(&self, col: usize) -> Result<i32> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Int32 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const i32) })
    }

    /// 按列索引读取i64
    pub fn get_i64(&self, col: usize) -> Result<i64> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Int64 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const i64) })
    }

    /// 按列索引读取f32
    pub fn get_f32(&self, col: usize) -> Result<f32> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Float32 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const f32) })
    }

    /// 按列索引读取f64
    pub fn get_f64(&self, col: usize) -> Result<f64> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Float64 {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const f64) })
    }

    /// 按列索引读取bool
    pub fn get_bool(&self, col: usize) -> Result<bool> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Bool {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe { core::ptr::read_unaligned(self.field_ptr(field) as *const u8) != 0 })
    }

    /// 按列索引读取时间戳
    pub fn get_timestamp(&self, col: usize) -> Result<crate::types::db_timestamp> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Timestamp && field.data_type != DataType::TimestampTZ {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe {
            core::ptr::read_unaligned(self.field_ptr(field) as *const crate::types::db_timestamp)
        })
    }

    /// 按列索引读取时间间隔
    pub fn get_interval(&self, col: usize) -> Result<crate::types::db_interval> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Interval {
            return Err(RemDbError::TypeMismatch);
        }
        Ok(unsafe {
            core::ptr::read_unaligned(self.field_ptr(field) as *const crate::types::db_interval)
        })
    }

    /// 按列索引读取字符串（零拷贝）
    pub fn get_str(&self, col: usize) -> Result<&'a str> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::VarChar && field.data_type != DataType::Char && field.data_type != DataType::Text {
            return Err(RemDbError::TypeMismatch);
        }
        let bytes = unsafe { core::slice::from_raw_parts(self.field_ptr(field), field.size) };
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        core::str::from_utf8(&bytes[..end]).map_err(|_| RemDbError::TypeMismatch)
    }

    /// 按列索引读取原始字节切片（零拷贝）
    pub fn get_bytes(&self, col: usize) -> Result<&'a [u8]> {
        let field = self.field_def(col)?;
        Ok(unsafe { core::slice::from_raw_parts(self.field_ptr(field), field.size) })
    }

    /// 按列索引读取JSON数据
    pub fn get_json(&self, col: usize) -> Result<crate::json::JsonDocument> {
        let field = self.field_def(col)?;
        if field.data_type != DataType::Json {
            return Err(RemDbError::TypeMismatch);
        }
        
        // 读取JsonStorage
        let json_storage = unsafe {
            core::ptr::read_unaligned(self.field_ptr(field) as *const crate::types::JsonStorage)
        };
        
        match json_storage {
            crate::types::JsonStorage::Inline(data) => {
                // 从内联存储创建JsonDocument
                let size = field.size;
                let data_slice = &data[..size];
                crate::json::JsonDocument::from_binary(data_slice, size)
                    .map_err(|_| RemDbError::TypeMismatch)
            }
            crate::types::JsonStorage::External { pool_id, offset, length } => {
                // 从外部存储创建JsonDocument
                let pool_manager = crate::json::memory_pool::get_global_json_pool_manager()
                    .ok_or(RemDbError::UnsupportedOperation)?;
                
                let pool = pool_manager.get_pool(pool_id)
                    .ok_or(RemDbError::UnsupportedOperation)?;
                
                if let Some(data_ptr) = pool.get_block_data(offset as usize, 0) {
                    let data_slice = unsafe {
                        core::slice::from_raw_parts(data_ptr, length as usize)
                    };
                    
                    crate::json::JsonDocument::from_binary(data_slice, length as usize)
                        .map_err(|_| RemDbError::TypeMismatch)
                } else {
                    Err(RemDbError::UnsupportedOperation)
                }
            }
            crate::types::JsonStorage::Null => {
                Ok(crate::json::JsonDocument::from_binary(&[], 0)
                    .map_err(|_| RemDbError::TypeMismatch)?)
            }
        }
    }
}

/// 记录游标（全表扫描）
pub struct RecordCursor<'a> {
    table: &'a MemoryTable,
    next_id: usize,
}

impl<'a> RecordCursor<'a> {
    fn new(table: &'a MemoryTable) -> Self {
        Self { table, next_id: 0 }
    }
}

impl<'a> Iterator for RecordCursor<'a> {
    type Item = RecordRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.next_id < self.table.def.max_records {
            let current = self.next_id;
            self.next_id += 1;
            unsafe {
                let status_ptr = self.table.status_array.as_ptr().add(current);
                if (*status_ptr).status == RecordStatus::Used {
                    let record_ptr = self.table.get_record_ptr(current);
                    return Some(RecordRef {
                        table: self.table,
                        id: current,
                        record_ptr,
                    });
                }
            }
        }
        None
    }
}

/// 记录ID游标（基于索引结果）
pub struct RecordIdCursor<'a> {
    table: &'a MemoryTable,
    ids: Vec<usize>,
    pos: usize,
}

impl<'a> RecordIdCursor<'a> {
    fn new(table: &'a MemoryTable, ids: Vec<usize>) -> Self {
        Self { table, ids, pos: 0 }
    }
}

impl<'a> Iterator for RecordIdCursor<'a> {
    type Item = RecordRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.ids.len() {
            let id = self.ids[self.pos];
            self.pos += 1;
            if id >= self.table.def.max_records {
                continue;
            }
            unsafe {
                let status_ptr = self.table.status_array.as_ptr().add(id);
                if (*status_ptr).status == RecordStatus::Used {
                    let record_ptr = self.table.get_record_ptr(id);
                    return Some(RecordRef {
                        table: self.table,
                        id,
                        record_ptr,
                    });
                }
            }
        }
        None
    }
}

// 添加Drop trait实现，用于释放动态分配的内存
impl Drop for MemoryTable {
    fn drop(&mut self) {
        unsafe {
            // 释放数据内存
            crate::memory::allocator::free(self.data_start);
            // 释放状态数组内存
            crate::memory::allocator::free(self.status_array.cast());
            // 释放空闲槽栈内存
            crate::memory::allocator::free(self.free_slots.cast());
        }
    }
}

impl MemoryTable {
    /// 创建新的内存表
    pub fn new(def: alloc::sync::Arc<TableDef>) -> Result<Self> {
        Self::new_with_options(def, false)
    }

    /// 创建新的内存表（带选项）
    pub fn new_with_options(def: alloc::sync::Arc<TableDef>, skip_status_init: bool) -> Result<Self> {
        // 确保max_records至少为1，避免创建无法使用的表
        if def.max_records == 0 {
            return Err(RemDbError::ConfigError);
        }

        // 计算所需内存大小
        let data_size = def.record_size * def.max_records;
        let status_size = core::mem::size_of::<RecordHeader>() * def.max_records;
        let free_slots_size = core::mem::size_of::<usize>() * def.max_records;

        // 动态分配内存
        let data_start = crate::memory::allocator::alloc(data_size)?;
        let status_start = crate::memory::allocator::alloc(status_size)?;
        let free_slots_start = crate::memory::allocator::alloc(free_slots_size)?;

        // 初始化状态数组
        let mut free_slot_count = def.max_records;
        unsafe {
            if !skip_status_init {
                let status_array = status_start.cast::<RecordHeader>();
                for i in 0..def.max_records {
                    let status_ptr = status_array.as_ptr().add(i);
                    (*status_ptr).status = RecordStatus::Free;
                    (*status_ptr).version = 0;
                    (*status_ptr).lock_type = crate::types::LockType::None;
                    (*status_ptr).lock_owner = 0;
                    (*status_ptr).lock_count = 0;
                }

                // 初始化空闲记录槽栈，将所有记录槽压入栈中
                let free_slots = free_slots_start.cast::<usize>();
                for i in 0..def.max_records {
                    *free_slots.as_ptr().add(i) = (def.max_records - 1 - i) as usize;
                }
            } else {
                // 跳过状态初始化，由WAL恢复过程处理
                free_slot_count = 0;
                // 初始化free_slots数组为0，避免未初始化内存访问
                let free_slots = free_slots_start.cast::<usize>();
                for i in 0..def.max_records {
                    *free_slots.as_ptr().add(i) = 0;
                }
            }
        }

        Ok(MemoryTable {
            def: def.clone(),
            data_start,
            status_array: status_start.cast(),
            record_count: 0,
            lock: 0,
            record_size: def.record_size, // 使用表定义中已经计算好的record_size
            free_slots: free_slots_start.cast(),
            free_slot_count: free_slot_count,
            low_power_mode: false,       // 默认不启用低功耗模式
            low_power_max_records: None, // 默认使用表定义的最大记录数
            snapshot_version: 0,         // 初始快照版本为0
            max_pk: 0,                   // 初始最大主键值为0
        })
    }

    /// 计算表所需的总内存大小
    pub const fn calculate_memory_size(def: &TableDef) -> usize {
        // 数据大小：记录大小 * 最大记录数
        let data_size = def.record_size * def.max_records;
        // 状态数组大小：RecordHeader大小 * 最大记录数
        let status_size = core::mem::size_of::<RecordHeader>() * def.max_records;
        // 空闲槽栈大小：usize大小 * 最大记录数
        let free_slots_size = core::mem::size_of::<usize>() * def.max_records;

        data_size + status_size + free_slots_size
    }

    /// 验证记录的约束
    pub unsafe fn validate_constraints(
        &self,
        record_data: *const u8,
        exclude_slot: Option<usize>,
    ) -> Result<()> {
        // 验证非空约束
        for field in self.def.fields.iter() {
            if field.not_null {
                // 检查字段是否为空
                let is_null = match field.data_type {
                    DataType::VarChar | DataType::Char | DataType::Text => {
                        // 检查字符串是否为空（全0）
                        let str_ptr = record_data.add(field.offset) as *const u8;
                        let mut all_zero = true;
                        for i in 0..field.size {
                            if *str_ptr.add(i) != 0 {
                                all_zero = false;
                                break;
                            }
                        }
                        all_zero
                    }
                    // 对于其他类型，我们需要检查是否使用了默认的零值作为null标记
                    // 这需要结合具体的业务逻辑和数据存储方式来判断
                    // 当前实现：检查是否有默认值，如果没有默认值且字段为NOT NULL，则验证该字段
                    // 注意：这里我们假设所有NOT NULL字段都应该有有效的值，而不是默认的零值
                    // 对于数值类型，0是合法值，所以我们不检查数值类型的null约束
                    // 对于布尔类型，false是合法值，所以我们不检查布尔类型的null约束
                    // 对于时间戳类型，0表示1970-01-01，是合法值，所以我们不检查时间戳类型的null约束
                    // 只有字符串类型需要检查是否为空
                    _ => false,
                };
                if is_null {
                    return Err(RemDbError::NotNullViolation);
                }
            }

            // 验证数值类型的有效性
            match field.data_type {
                DataType::Float32 => {
                    let value =
                        core::ptr::read_unaligned(record_data.add(field.offset) as *const f32);
                    if value.is_nan() || value.is_infinite() {
                        return Err(RemDbError::TypeMismatch);
                    }
                }
                DataType::Float64 => {
                    let value =
                        core::ptr::read_unaligned(record_data.add(field.offset) as *const f64);
                    if value.is_nan() || value.is_infinite() {
                        return Err(RemDbError::TypeMismatch);
                    }
                }
                _ => {}
            }
        }

        // 验证主键唯一性约束
        if !self.def.primary_key.is_empty() {
            // 遍历所有记录，检查是否存在重复主键
            for slot_id in 0..self.def.max_records {
                // 跳过要排除的槽位（用于更新操作）
                if exclude_slot == Some(slot_id) {
                    continue;
                }

                let status_ptr = self.status_array.as_ptr().add(slot_id);
                let status = &*status_ptr;

                // 只检查已使用且可见的记录（考虑MVCC）
                if status.status == RecordStatus::Used {
                    // 获取当前事务ID
                    let current_tx_id = crate::transaction::tx_id_counter();

                    // 检查记录是否可见
                    let is_visible = crate::transaction::is_visible(
                        status.create_tx_id,
                        status.delete_tx_id,
                        current_tx_id,
                    );

                    if !is_visible {
                        continue;
                    }
                    // 获取记录数据指针
                    let record_ptr = self.data_start.as_ptr().add(slot_id * self.record_size);

                    // 比较所有主键字段的值
                    let mut is_duplicate = true;
                    for &pk_col_idx in &self.def.primary_key {
                        let pk_field = &self.def.fields[pk_col_idx];
                        let fields_equal = match pk_field.data_type {
                        DataType::UInt8 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const u8,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const u8,
                            );
                            current == existing
                        }
                        DataType::UInt16 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const u16,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const u16,
                            );
                            current == existing
                        }
                        DataType::UInt32 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const u32,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const u32,
                            );
                            current == existing
                        }
                        DataType::UInt64 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const u64,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const u64,
                            );
                            current == existing
                        }
                        DataType::Int8 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const i8,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const i8,
                            );
                            current == existing
                        }
                        DataType::Int16 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const i16,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const i16,
                            );
                            current == existing
                        }
                        DataType::Int32 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const i32,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const i32,
                            );
                            current == existing
                        }
                        DataType::Int64 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const i64,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const i64,
                            );
                            current == existing
                        }
                        DataType::Float32 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const f32,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const f32,
                            );
                            current == existing
                        }
                        DataType::Float64 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const f64,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const f64,
                            );
                            current == existing
                        }
                        DataType::Bool => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset) as *const bool,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(pk_field.offset) as *const bool,
                            );
                            current == existing
                        }
                        DataType::Timestamp => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset)
                                    as *const crate::types::db_timestamp,
                            );
                            let existing =
                                core::ptr::read_unaligned(record_ptr.add(pk_field.offset)
                                    as *const crate::types::db_timestamp);
                            current.value == existing.value
                        }
                        DataType::TimestampTZ => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset)
                                    as *const crate::types::db_timestamp,
                            );
                            let existing =
                                core::ptr::read_unaligned(record_ptr.add(pk_field.offset)
                                    as *const crate::types::db_timestamp);
                            current.value == existing.value
                                && current.tz_offset == existing.tz_offset
                        }
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            // 比较字符串内容
                            let current_str = 
                                record_data.add(pk_field.offset) as *const u8;
                            let existing_str = 
                                record_ptr.add(pk_field.offset) as *const u8;
                            let mut is_equal = true;
                            for i in 0..pk_field.size {
                                if *current_str.add(i) != *existing_str.add(i) {
                                    is_equal = false;
                                    break;
                                }
                            }
                            is_equal
                        }
                        DataType::Interval => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(pk_field.offset)
                                    as *const crate::types::db_interval,
                            );
                            let existing =
                                core::ptr::read_unaligned(record_ptr.add(pk_field.offset)
                                    as *const crate::types::db_interval);
                            current.value == existing.value
                        }
                        DataType::Vector => false, // 向量字段暂不支持作为主键
                        DataType::Json => false, // JSON字段暂不支持作为主键
                        };
                        
                        // 如果有任何一个主键字段不相等，则不是重复记录
                        if !fields_equal {
                            is_duplicate = false;
                            break;
                        }
                    }

                    if is_duplicate {
                        return Err(RemDbError::DuplicateKey);
                    }
                }
            }
        }

        // 验证唯一约束（UNIQUE）
        for unique_field in self.def.fields.iter().filter(|f| f.unique) {
            // 跳过主键字段，因为已经检查过了
            if unique_field.primary_key {
                continue;
            }

            // 遍历所有记录，检查是否存在重复值
            for slot_id in 0..self.def.max_records {
                // 跳过要排除的槽位（用于更新操作）
                if exclude_slot == Some(slot_id) {
                    continue;
                }

                let status_ptr = self.status_array.as_ptr().add(slot_id);
                let status = &*status_ptr;

                // 只检查已使用且可见的记录（考虑MVCC）
                if status.status == RecordStatus::Used {
                    // 获取当前事务ID
                    let current_tx_id = crate::transaction::tx_id_counter();

                    // 检查记录是否可见
                    let is_visible = crate::transaction::is_visible(
                        status.create_tx_id,
                        status.delete_tx_id,
                        current_tx_id,
                    );

                    if !is_visible {
                        continue;
                    }
                    // 获取记录数据指针
                    let record_ptr = self.data_start.as_ptr().add(slot_id * self.record_size);

                    // 根据字段类型比较值
                    let is_duplicate = match unique_field.data_type {
                        DataType::UInt8 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const u8,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const u8,
                            );
                            current == existing
                        }
                        DataType::UInt16 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const u16,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const u16,
                            );
                            current == existing
                        }
                        DataType::UInt32 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const u32,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const u32,
                            );
                            current == existing
                        }
                        DataType::UInt64 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const u64,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const u64,
                            );
                            current == existing
                        }
                        DataType::Int8 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const i8,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const i8,
                            );
                            current == existing
                        }
                        DataType::Int16 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const i16,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const i16,
                            );
                            current == existing
                        }
                        DataType::Int32 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const i32,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const i32,
                            );
                            current == existing
                        }
                        DataType::Int64 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const i64,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const i64,
                            );
                            current == existing
                        }
                        DataType::Float32 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const f32,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const f32,
                            );
                            current == existing
                        }
                        DataType::Float64 => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const f64,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const f64,
                            );
                            current == existing
                        }
                        DataType::Bool => {
                            let current = core::ptr::read_unaligned(
                                record_data.add(unique_field.offset) as *const bool,
                            );
                            let existing = core::ptr::read_unaligned(
                                record_ptr.add(unique_field.offset) as *const bool,
                            );
                            current == existing
                        }
                        DataType::Timestamp => {
                            let current =
                                core::ptr::read_unaligned(record_data.add(unique_field.offset)
                                    as *const crate::types::db_timestamp);
                            let existing =
                                core::ptr::read_unaligned(record_ptr.add(unique_field.offset)
                                    as *const crate::types::db_timestamp);
                            current.value == existing.value
                        }
                        DataType::TimestampTZ => {
                            let current =
                                core::ptr::read_unaligned(record_data.add(unique_field.offset)
                                    as *const crate::types::db_timestamp);
                            let existing =
                                core::ptr::read_unaligned(record_ptr.add(unique_field.offset)
                                    as *const crate::types::db_timestamp);
                            current.value == existing.value
                                && current.tz_offset == existing.tz_offset
                        }
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            // 比较字符串内容
                            let current_str = record_data.add(unique_field.offset) as *const u8;
                            let existing_str = record_ptr.add(unique_field.offset) as *const u8;
                            let mut is_equal = true;
                            for i in 0..unique_field.size {
                                if *current_str.add(i) != *existing_str.add(i) {
                                    is_equal = false;
                                    break;
                                }
                            }
                            is_equal
                        }
                        DataType::Interval => {
                            let current =
                                core::ptr::read_unaligned(record_data.add(unique_field.offset)
                                    as *const crate::types::db_interval);
                            let existing =
                                core::ptr::read_unaligned(record_ptr.add(unique_field.offset)
                                    as *const crate::types::db_interval);
                            current.value == existing.value
                        }
                        DataType::Vector => false, // 向量字段暂不支持唯一约束
                        DataType::Json => false, // JSON字段暂不支持唯一约束
                    };

                    if is_duplicate {
                        return Err(RemDbError::DuplicateKey);
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取字段值的辅助方法（按偏移量）
    unsafe fn get_field_by_offset(
        &self,
        record_data: *const u8,
        offset: usize,
        data_type: DataType,
        size: usize,
    ) -> Result<Value> {
        let field_ptr = record_data.add(offset);

        let value = match data_type {
            DataType::UInt8 => Value {
                u8: *field_ptr as u8,
            },
            DataType::UInt16 => Value {
                u16: core::ptr::read_unaligned(field_ptr as *const u16),
            },
            DataType::UInt32 => Value {
                u32: core::ptr::read_unaligned(field_ptr as *const u32),
            },
            DataType::UInt64 => Value {
                u64: core::ptr::read_unaligned(field_ptr as *const u64),
            },
            DataType::Int8 => Value {
                i8: core::ptr::read_unaligned(field_ptr as *const i8),
            },
            DataType::Int16 => Value {
                i16: core::ptr::read_unaligned(field_ptr as *const i16),
            },
            DataType::Int32 => Value {
                i32: core::ptr::read_unaligned(field_ptr as *const i32),
            },
            DataType::Int64 => Value {
                i64: core::ptr::read_unaligned(field_ptr as *const i64),
            },
            DataType::Float32 => Value {
                float32: core::ptr::read_unaligned(field_ptr as *const f32),
            },
            DataType::Float64 => Value {
                float64: core::ptr::read_unaligned(field_ptr as *const f64),
            },
            DataType::Bool => Value {
                bool: *field_ptr != 0,
            },
            DataType::Timestamp => Value {
                time: core::ptr::read_unaligned(field_ptr as *const crate::types::db_timestamp),
            },
            DataType::TimestampTZ => Value {
                time: core::ptr::read_unaligned(field_ptr as *const crate::types::db_timestamp),
            },
            DataType::Interval => Value {
                interval: core::ptr::read_unaligned(field_ptr as *const crate::types::db_interval),
            },
            DataType::VarChar | DataType::Char | DataType::Text => {
                let mut str_value = [0u8; crate::types::MAX_STRING_LEN];
                let copy_size = core::cmp::min(size, crate::types::MAX_STRING_LEN);
                memcpy(str_value.as_mut_ptr(), field_ptr, copy_size);
                Value { string: str_value }
            }
            DataType::Vector => {
                Value { vector: field_ptr as *const f32 }
            },
            DataType::Json => {
                // 从存储中读取JsonStorage
                let json_storage = core::ptr::read_unaligned(field_ptr as *const crate::types::JsonStorage);
                Value { json_storage }
            },
        };

        Ok(value)
    }

    /// 插入记录
    pub fn insert(&mut self, record_data: *const u8) -> Result<usize> {
        // 增加写入操作计数
        crate::get_global_db().map(|db| db.metrics.inc_write_ops());

        // 验证约束
        unsafe {
            self.validate_constraints(record_data, None)?;
        }

        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        let lock_ptr = &mut self.lock;
        defer! { crate::platform::spin_unlock(lock_ptr); }

        // 检查是否已满
        let max_records = if self.low_power_mode {
            self.low_power_max_records.unwrap_or(self.def.max_records)
        } else {
            self.def.max_records
        };

        let mut slot_id = 0;
        let mut is_overwrite = false;

        if self.record_count >= max_records {
            if self.low_power_mode {
                // 低功耗模式：覆盖最旧的记录
                // 查找最旧的记录
                let mut oldest_id = None;
                let mut oldest_version = u16::MAX;

                for i in 0..self.def.max_records {
                    unsafe {
                        let status_ptr = self.status_array.as_ptr().add(i);
                        let status = &*status_ptr;
                        if status.status == RecordStatus::Used && status.version < oldest_version {
                            oldest_id = Some(i);
                            oldest_version = status.version;
                        }
                    }
                }

                let slot_id_val = match oldest_id {
                    Some(id) => id,
                    None => return Err(RemDbError::NoRecordsToOverwrite),
                };

                slot_id = slot_id_val;
                is_overwrite = true;
            } else {
                // 正常模式：返回错误
                return Err(RemDbError::OutOfMemory);
            }
        } else {
            // 从空闲槽栈获取空闲记录槽（O(1)时间复杂度）
            if self.free_slot_count == 0 {
                return Err(RemDbError::OutOfMemory);
            }

            // 获取栈顶空闲槽
            slot_id = unsafe {
                self.free_slot_count -= 1;
                *self.free_slots.as_ptr().add(self.free_slot_count)
            };
        }

        // 计算记录地址
        let record_ptr = unsafe { self.data_start.as_ptr().add(slot_id * self.record_size) };

        // 记录日志（如果有活跃事务）
        if crate::transaction::has_active_tx() {
            // 保存新数据
            let mut new_data = Vec::with_capacity(self.record_size);
            new_data.resize(self.record_size, 0);
            memcpy(new_data.as_mut_ptr(), record_data, self.record_size);

            // 检查当前事务是否有效，避免访问悬空指针
            unsafe {
                if let Some(mut tx) = crate::transaction::get_current_tx() {
                    let tx_id = tx.as_mut().id;
                    // 使用begin_log_item将日志项添加到事务的日志缓冲区
                    tx.as_mut().begin_log_item(
                        tx_id,
                        crate::transaction::LogOperation::Insert,
                        self.def.id,
                        slot_id as u16,
                        self.record_size as u16,
                        None,
                        Some(&new_data),
                    );
                }
            }
        }

        // 拷贝记录数据
        memcpy(record_ptr, record_data, self.record_size);

        // 更新状态
        let status_ptr = unsafe { self.status_array.as_ptr().add(slot_id) };
        unsafe {
            (*status_ptr).status = RecordStatus::Used;
            (*status_ptr).version += 1;
        }

        // 更新最大主键值（仅当主键是单个整数类型字段时）
        if self.def.primary_key.len() == 1 {
            let pk_col_idx = self.def.primary_key[0];
            if let Some(pk_field) = self.def.fields.get(pk_col_idx) {
                let pk_value = unsafe {
                    match pk_field.data_type {
                        DataType::UInt8 => *record_ptr.add(pk_field.offset) as u64,
                        DataType::UInt16 => {
                            core::ptr::read_unaligned(record_ptr.add(pk_field.offset) as *const u16)
                                as u64
                        }
                        DataType::UInt32 => {
                            core::ptr::read_unaligned(record_ptr.add(pk_field.offset) as *const u32)
                                as u64
                        }
                        DataType::UInt64 => {
                            core::ptr::read_unaligned(record_ptr.add(pk_field.offset) as *const u64)
                        }
                        DataType::Int8 => *record_ptr.add(pk_field.offset) as i8 as u64,
                        DataType::Int16 => {
                            core::ptr::read_unaligned(record_ptr.add(pk_field.offset) as *const i16)
                                as u64
                        }
                        DataType::Int32 => {
                            core::ptr::read_unaligned(record_ptr.add(pk_field.offset) as *const i32)
                                as u64
                        }
                        DataType::Int64 => {
                            core::ptr::read_unaligned(record_ptr.add(pk_field.offset) as *const i64)
                                as u64
                        }
                        _ => 0, // 非整数类型主键不更新max_pk
                    }
                };

                if pk_value > self.max_pk {
                    self.max_pk = pk_value;
                }
            }
        }

        // 更新记录计数（如果是覆盖旧记录，不需要增加计数）
        if !is_overwrite {
            self.record_count += 1;
            // 更新内存使用：增加一条记录的内存
            crate::get_global_db().map(|db| db.metrics.add_used_memory(self.record_size));
        }

        let inserted_slot_id = slot_id;

        // 释放锁后发布到pubsub
        Ok(inserted_slot_id)
    }

    // 内联publish_to_pubsub逻辑，避免borrow checker问题
    #[cfg(feature = "pubsub")]
    unsafe fn publish_to_pubsub_inline(
        table_name: &str,
        record_size: usize,
        id: usize,
        record_data: *const u8,
        is_insert: bool,
    ) {
        let table_topic = crate::pubsub::topics::get_table_content_topic(table_name);

        // 获取主题ID
        if let Some(topic_id) = crate::pubsub::get_topic_id(&table_topic) {
            // 构建消息
            let op_type = if is_insert { "INSERT" } else { "UPDATE" };
            let mut msg = alloc::format!("{}:table={},id={},data=", op_type, table_name, id);

            // 添加记录数据（hex格式）
            for i in 0..record_size {
                let byte = *record_data.add(i);
                msg.push_str(&format!("{:02x}", byte));
            }

            // 发布到pubsub
            let _ = crate::pubsub::publish(topic_id, msg.as_bytes());
        }
    }

    /// 更新记录
    pub unsafe fn update(&mut self, id: usize, record_data: *const u8) -> Result<()> {
        // 增加更新操作计数
        crate::get_global_db().map(|db| db.metrics.inc_update_ops());

        // 检查ID有效性
        if id >= self.def.max_records {
            return Err(RemDbError::RecordNotFound);
        }

        // 获取状态指针（无锁，因为只是读取）
        let status_ptr = self.status_array.as_ptr().add(id);
        if (*status_ptr).status != RecordStatus::Used {
            return Err(RemDbError::RecordNotFound);
        }

        // 验证约束（排除当前记录）
        self.validate_constraints(record_data, Some(id))?;

        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        let lock_ptr = &mut self.lock;
        defer! { crate::platform::spin_unlock(lock_ptr); }

        // 计算记录地址
        let record_ptr = self.data_start.as_ptr().add(id * self.record_size);

        // 记录日志（如果有活跃事务）
        if crate::transaction::has_active_tx() {
            // 保存旧数据
            let mut old_data = Vec::with_capacity(self.record_size);
            old_data.resize(self.record_size, 0);
            memcpy(old_data.as_mut_ptr(), record_ptr, self.record_size);

            // 保存新数据
            let mut new_data = Vec::with_capacity(self.record_size);
            new_data.resize(self.record_size, 0);
            memcpy(new_data.as_mut_ptr(), record_data, self.record_size);

            // 检查当前事务是否有效，避免访问悬空指针
            if let Some(mut tx) = crate::transaction::get_current_tx() {
                unsafe {
                    let tx_id = tx.as_mut().id;
                    // 使用begin_log_item将日志项添加到事务的日志缓冲区
                    tx.as_mut().begin_log_item(
                        tx_id,
                        crate::transaction::LogOperation::Update,
                        self.def.id,
                        id as u16,
                        self.record_size as u16,
                        Some(&old_data),
                        Some(&new_data),
                    );
                }
            }
        }

        // 更新记录数据
        memcpy(record_ptr, record_data, self.record_size);

        // 更新版本号
        (*status_ptr).version += 1;

        Ok(())
    }

    /// 删除记录
    pub unsafe fn delete(&mut self, id: usize) -> Result<()> {
        // 增加删除操作计数
        crate::get_global_db().map(|db| db.metrics.inc_delete_ops());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }

        // 检查ID有效性
        if id >= self.def.max_records {
            return Err(RemDbError::RecordNotFound);
        }

        let status_ptr = self.status_array.as_ptr().add(id);
        if (*status_ptr).status != RecordStatus::Used {
            return Err(RemDbError::RecordNotFound);
        }

        if crate::transaction::has_active_tx() {
            let record_ptr = self.data_start.as_ptr().add(id * self.record_size);
            let mut old_data = Vec::with_capacity(self.record_size);
            old_data.resize(self.record_size, 0);
            memcpy(old_data.as_mut_ptr(), record_ptr, self.record_size);

            if let Some(mut tx) = crate::transaction::get_current_tx() {
                unsafe {
                    let tx_id = tx.as_mut().id;
                    // 使用begin_log_item将日志项添加到事务的日志缓冲区
                    tx.as_mut().begin_log_item(
                        tx_id,
                        crate::transaction::LogOperation::Delete,
                        self.def.id,
                        id as u16,
                        self.record_size as u16,
                        Some(&old_data),
                        None,
                    );
                }
            }
        }

        // 标记为空闲
        (*status_ptr).status = RecordStatus::Free;
        (*status_ptr).version += 1;

        // 清空记录数据
        let record_ptr = self.data_start.as_ptr().add(id * self.record_size);
        memset(record_ptr, 0, self.record_size);

        // 将空闲槽压回栈中，确保不超过数组大小
        if self.free_slot_count < self.def.max_records {
            *self.free_slots.as_ptr().add(self.free_slot_count) = id;
            self.free_slot_count += 1;
        }

        // 更新记录计数
        self.record_count -= 1;

        // 更新内存使用：减少一条记录的内存
        crate::get_global_db().map(|db| db.metrics.sub_used_memory(self.record_size));

        Ok(())
    }

    /// 根据ID获取记录
    pub unsafe fn get_by_id(&self, id: usize, dest: *mut u8) -> Result<()> {
        // 增加读取操作计数
        crate::get_global_db().map(|db| db.metrics.inc_read_ops());
        // 检查ID有效性
        if id >= self.def.max_records {
            return Err(RemDbError::RecordNotFound);
        }

        let status_ptr = self.status_array.as_ptr().add(id);
        if (*status_ptr).status != RecordStatus::Used {
            return Err(RemDbError::RecordNotFound);
        }

        // 拷贝记录数据
        let record_ptr = self.data_start.as_ptr().add(id * self.record_size);
        memcpy(dest, record_ptr, self.record_size);

        Ok(())
    }

    /// 根据ID获取记录引用（零拷贝）
    pub fn get_by_id_ref(&self, id: usize) -> Option<RecordRef<'_>> {
        // 增加读取操作计数
        crate::get_global_db().map(|db| db.metrics.inc_read_ops());
        if id >= self.def.max_records {
            return None;
        }
        unsafe {
            let status_ptr = self.status_array.as_ptr().add(id);
            if (*status_ptr).status != RecordStatus::Used {
                return None;
            }
            let record_ptr = self.data_start.as_ptr().add(id * self.record_size);
            Some(RecordRef {
                table: self,
                id,
                record_ptr,
            })
        }
    }

    /// 扫描游标（零拷贝）
    pub fn scan_ref(&self) -> RecordCursor<'_> {
        RecordCursor::new(self)
    }

    /// 基于记录ID列表的游标（零拷贝）
    pub fn scan_ids_ref(&self, ids: Vec<usize>) -> RecordIdCursor<'_> {
        RecordIdCursor::new(self, ids)
    }

    /// 发布表数据变更到pubsub

    #[cfg(feature = "pubsub")]
    unsafe fn publish_to_pubsub(&self, id: usize, record_data: *const u8, is_insert: bool) {
        let table_name = &self.def.name;
        let table_topic = crate::pubsub::topics::get_table_content_topic(table_name);

        // 获取主题ID
        if let Some(topic_id) = crate::pubsub::get_topic_id(&table_topic) {
            // 构建消息
            let op_type = if is_insert { "INSERT" } else { "UPDATE" };
            let mut msg = alloc::format!("{}:table={},id={},data=", op_type, table_name, id);

            // 添加记录数据（hex格式）
            for i in 0..self.record_size {
                let byte = *record_data.add(i);
                msg.push_str(&format!("{:02x}", byte));
            }

            // 发布到pubsub
            let _ = crate::pubsub::publish(topic_id, msg.as_bytes());
        }
    }

    /// 获取字段值
    pub unsafe fn get_field(&self, record_data: *const u8, field_index: usize) -> Result<Value> {
        // 检查字段索引有效性
        if field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let field = &self.def.fields[field_index];
        let field_ptr = record_data.add(field.offset);

        // 根据字段类型获取值
        let value = match field.data_type {
            crate::types::DataType::UInt8 => Value {
                u8: *field_ptr as u8,
            },
            crate::types::DataType::UInt16 => Value {
                u16: core::ptr::read_unaligned(field_ptr as *const u16),
            },
            crate::types::DataType::UInt32 => Value {
                u32: core::ptr::read_unaligned(field_ptr as *const u32),
            },
            crate::types::DataType::UInt64 => Value {
                u64: core::ptr::read_unaligned(field_ptr as *const u64),
            },
            crate::types::DataType::Int8 => Value {
                i8: core::ptr::read_unaligned(field_ptr as *const i8),
            },
            crate::types::DataType::Int16 => Value {
                i16: core::ptr::read_unaligned(field_ptr as *const i16),
            },
            crate::types::DataType::Int32 => Value {
                i32: core::ptr::read_unaligned(field_ptr as *const i32),
            },
            crate::types::DataType::Int64 => Value {
                i64: core::ptr::read_unaligned(field_ptr as *const i64),
            },
            crate::types::DataType::Float32 => Value {
                float32: core::ptr::read_unaligned(field_ptr as *const f32),
            },
            crate::types::DataType::Float64 => Value {
                float64: core::ptr::read_unaligned(field_ptr as *const f64),
            },
            crate::types::DataType::Bool => Value {
                bool: *field_ptr != 0,
            },
            crate::types::DataType::Timestamp => Value {
                time: core::ptr::read_unaligned(field_ptr as *const crate::types::db_timestamp),
            },
            crate::types::DataType::TimestampTZ => Value {
                time: core::ptr::read_unaligned(field_ptr as *const crate::types::db_timestamp),
            },
            crate::types::DataType::VarChar | crate::types::DataType::Char | crate::types::DataType::Text => {
                let mut str_value = [0u8; crate::types::MAX_STRING_LEN];
                // 只复制不超过MAX_STRING_LEN的字节，避免缓冲区溢出
                let copy_size = core::cmp::min(field.size, crate::types::MAX_STRING_LEN);
                memcpy(str_value.as_mut_ptr(), field_ptr, copy_size);
                Value { string: str_value }
            }
            crate::types::DataType::Interval => Value {
                interval: core::ptr::read_unaligned(field_ptr as *const crate::types::db_interval),
            },
            crate::types::DataType::Vector => Value {
                vector: field_ptr as *const f32,
            },
            crate::types::DataType::Json => {
                // 从存储中读取JsonStorage
                let json_storage = core::ptr::read_unaligned(field_ptr as *const crate::types::JsonStorage);
                Value { json_storage }
            },
        };

        Ok(value)
    }

    /// 设置字段值
    pub unsafe fn set_field(
        &self,
        record_data: *mut u8,
        field_index: usize,
        value: &Value,
    ) -> Result<()> {
        // 检查字段索引有效性
        if field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let field = &self.def.fields[field_index];
        let field_ptr = record_data.add(field.offset);

        // 根据字段类型设置值
        match field.data_type {
            crate::types::DataType::UInt8 => {
                *(field_ptr as *mut u8) = value.u8;
            }
            crate::types::DataType::UInt16 => {
                *(field_ptr as *mut u16) = value.u16;
            }
            crate::types::DataType::UInt32 => {
                *(field_ptr as *mut u32) = value.u32;
            }
            crate::types::DataType::UInt64 => {
                *(field_ptr as *mut u64) = value.u64;
            }
            crate::types::DataType::Int8 => {
                *(field_ptr as *mut i8) = value.i8;
            }
            crate::types::DataType::Int16 => {
                *(field_ptr as *mut i16) = value.i16;
            }
            crate::types::DataType::Int32 => {
                *(field_ptr as *mut i32) = value.i32;
            }
            crate::types::DataType::Int64 => {
                *(field_ptr as *mut i64) = value.i64;
            }
            crate::types::DataType::Float32 => {
                *(field_ptr as *mut f32) = value.float32;
            }
            crate::types::DataType::Float64 => {
                *(field_ptr as *mut f64) = value.float64;
            }
            crate::types::DataType::Bool => {
                *field_ptr = if value.bool { 1 } else { 0 };
            }
            crate::types::DataType::Timestamp => {
                *(field_ptr as *mut crate::types::db_timestamp) = value.time;
            }
            crate::types::DataType::TimestampTZ => {
                *(field_ptr as *mut crate::types::db_timestamp) = value.time;
            }
            crate::types::DataType::VarChar | crate::types::DataType::Char | crate::types::DataType::Text => {
                memcpy(field_ptr, value.string.as_ptr(), field.size);
            }
            crate::types::DataType::Interval => {
                *(field_ptr as *mut crate::types::db_interval) = value.interval;
            }
            crate::types::DataType::Vector => {
                // 获取向量维度
                let vector_metadata = field.vector_metadata.as_ref().unwrap();
                let dimension = vector_metadata.dimension as usize;
                
                // 压缩向量数据后写入
                crate::compression::compress_vector(
                    value.vector,
                    dimension,
                    field_ptr
                );
            }
            crate::types::DataType::Json => {
                // 写入JsonStorage
                *(field_ptr as *mut crate::types::JsonStorage) = value.json_storage;
            }
        }

        Ok(())
    }

    /// 获取当前记录数
    pub fn record_count(&self) -> usize {
        self.record_count
    }

    /// 获取最大记录数
    pub fn max_records(&self) -> usize {
        self.def.max_records
    }

    /// 检查表是否已满
    pub fn is_full(&self) -> bool {
        self.record_count >= self.def.max_records
    }

    /// 设置低功耗模式
    pub fn set_low_power_mode(&mut self, enabled: bool, max_records: Option<usize>) {
        self.low_power_mode = enabled;
        self.low_power_max_records = max_records;
    }

    /// 检查是否处于低功耗模式
    pub fn is_low_power_mode(&self) -> bool {
        self.low_power_mode
    }

    /// 遍历记录（零拷贝迭代）
    ///
    /// # 功能说明
    /// 遍历表中所有已使用的记录，通过回调函数直接提供指向表内存的指针，实现零拷贝访问
    ///
    /// # 安全说明
    /// - 此方法提供原始指针给回调函数，调用者需要确保指针使用的安全性
    /// - 回调函数中获取的指针在迭代过程中有效
    /// - 并发访问时需要考虑线程安全
    /// - 请勿在回调函数外部长时间持有返回的指针
    /// - 迭代过程中修改表结构可能导致未定义行为
    ///
    /// # 使用场景
    /// - 全表扫描或范围查询
    /// - 批量数据处理
    /// - 数据导出或备份
    /// - 高性能数据分析
    ///
    /// # 参数
    /// - `f`: 回调函数，接收记录ID和记录数据指针，返回bool值指示是否继续迭代
    ///   - `id`: 记录在表中的唯一标识符
    ///   - `record_ptr`: 指向记录数据的原始指针
    ///   - 返回值: `true` 继续迭代，`false` 停止迭代
    ///
    /// # 返回值
    /// - `Result<()>`: 迭代操作的结果，成功返回`Ok(())`，失败返回错误信息
    /// ```
    /// // 示例：如何使用iterate方法遍历记录
    /// // 注意：此示例仅展示用法，实际使用时需要先创建MemoryTable实例
    ///
    /// // unsafe {
    /// //     // 使用iterate方法遍历记录
    /// //     table.iterate(|id, record_ptr| {
    /// //         // 直接访问记录数据
    /// //         let id_value = *(record_ptr as *const u32); // 第一个字段是id，偏移量为0
    /// //         let value_value = *(record_ptr.add(4) as *const u32); // 第二个字段是value，偏移量为4
    /// //         true // 继续迭代
    /// //     }).unwrap();
    /// // }
    /// ```
    pub unsafe fn iterate<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(usize, *const u8) -> bool,
    {
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status == RecordStatus::Used {
                let record_ptr = self.data_start.as_ptr().add(i * self.record_size);
                if !f(i, record_ptr) {
                    break;
                }
            }
        }

        Ok(())
    }

    /// 获取记录状态指针
    ///
    /// # 安全说明
    /// - 此方法返回原始指针，调用者需要确保指针使用的安全性
    /// - 索引必须在有效范围内（0 <= index < max_records）
    /// - 返回的指针在表被销毁或内存重分配前有效
    pub unsafe fn get_status_ptr(&self, index: usize) -> *mut RecordHeader {
        // 安全检查：确保索引在有效范围内
        debug_assert!(
            index < self.def.max_records,
            "Record index out of bounds: {} (max: {})",
            index,
            self.def.max_records
        );
        self.status_array.as_ptr().add(index)
    }

    /// 获取记录数据指针（零拷贝访问）
    ///
    /// # 功能说明
    /// 直接返回指向表内存中记录数据的原始指针，实现零拷贝访问
    ///
    /// # 安全说明
    /// - 此方法返回原始指针，调用者需要确保指针使用的安全性
    /// - 索引必须在有效范围内（0 <= index < max_records）
    /// - 返回的指针在表被销毁或内存重分配前有效
    /// - 并发访问时需要考虑线程安全
    /// - 请勿在事务外部长时间持有此指针
    ///
    /// # 使用场景
    /// - 需要极致性能的批量数据处理
    /// - 频繁访问同一条记录的多个字段
    /// - 与外部系统集成，需要直接内存访问
    ///
    /// # 示例
    /// ```
    /// // 示例：如何使用get_record_ptr方法获取记录指针
    /// // 注意：此示例仅展示用法，实际使用时需要先创建MemoryTable实例
    ///
    /// // unsafe {
    /// //     let record_id = 0; // 示例记录ID
    /// //     let field_offset = 4; // 示例字段偏移量（第二个字段，偏移量为4）
    /// //     
    /// //     // 使用get_record_ptr方法获取记录指针
    /// //     let record_ptr = table.get_record_ptr(record_id);
    /// //     // 直接访问记录数据，无需拷贝
    /// //     let value = *(record_ptr.add(field_offset) as *const u32);
    /// //     println!("Record {} value: {}", record_id, value);
    /// // }
    /// ```
    pub unsafe fn get_record_ptr(&self, index: usize) -> *const u8 {
        // 安全检查：确保索引在有效范围内
        debug_assert!(
            index < self.def.max_records,
            "Record index out of bounds: {} (max: {})",
            index,
            self.def.max_records
        );
        self.data_start.as_ptr().add(index * self.record_size)
    }

    /// 获取记录数据可变指针（零拷贝访问）
    ///
    /// # 功能说明
    /// 直接返回指向表内存中记录数据的可变原始指针，实现零拷贝访问和修改
    ///
    /// # 安全说明
    /// - 此方法返回原始可变指针，调用者需要确保指针使用的安全性
    /// - 索引必须在有效范围内（0 <= index < max_records）
    /// - 返回的指针在表被销毁或内存重分配前有效
    /// - 并发访问时需要考虑线程安全
    /// - 请勿在事务外部长时间持有此指针
    /// - 修改数据时请确保遵循ACID原则
    ///
    /// # 使用场景
    /// - 需要原地修改记录数据
    /// - 批量更新多条记录
    /// - 高性能数据处理
    ///
    /// # 示例
    /// ```
    /// // 示例：如何使用get_record_ptr_mut方法获取记录可变指针
    /// // 注意：此示例仅展示用法，实际使用时需要先创建MemoryTable实例
    ///
    /// // unsafe {
    /// //     let record_id = 0; // 示例记录ID
    /// //     let field_offset = 4; // 示例字段偏移量（第二个字段，偏移量为4）
    /// //     let new_value = 100u32; // 要设置的新值
    /// //     
    /// //     // 使用get_record_ptr_mut方法获取记录指针
    /// //     let record_ptr = table.get_record_ptr_mut(record_id);
    /// //     // 直接修改记录数据，无需拷贝
    /// //     *(record_ptr.add(field_offset) as *mut u32) = new_value;
    /// //     
    /// //     // 验证修改结果
    /// //     let updated_ptr = table.get_record_ptr(record_id);
    /// //     let updated_value = *(updated_ptr.add(field_offset) as *const u32);
    /// //     assert_eq!(updated_value, new_value);
    /// // }
    /// ```
    pub unsafe fn get_record_ptr_mut(&mut self, index: usize) -> *mut u8 {
        // 安全检查：确保索引在有效范围内
        debug_assert!(
            index < self.def.max_records,
            "Record index out of bounds: {} (max: {})",
            index,
            self.def.max_records
        );
        self.data_start.as_ptr().add(index * self.record_size) as *mut u8
    }

    /// 设置JSON字段值
    pub fn set_json(&mut self, record_data: *mut u8, col: usize, json_doc: &crate::json::JsonDocument) -> Result<()> {
        let field = self.def.fields.get(col)
            .ok_or(RemDbError::FieldNotFound)?;
        
        if field.data_type != DataType::Json {
            return Err(RemDbError::TypeMismatch);
        }
        
        let field_ptr = unsafe { record_data.add(field.offset) };
        
        match json_doc.storage() {
            crate::types::JsonStorage::Inline(data) => {
                // 写入内联存储
                unsafe {
                    let json_storage = crate::types::JsonStorage::Inline(*data);
                    core::ptr::write_unaligned(field_ptr as *mut crate::types::JsonStorage, json_storage);
                }
            }
            crate::types::JsonStorage::External { pool_id, offset, length } => {
                // 写入外部存储引用
                unsafe {
                    let json_storage = crate::types::JsonStorage::External {
                        pool_id: *pool_id,
                        offset: *offset,
                        length: *length,
                    };
                    core::ptr::write_unaligned(field_ptr as *mut crate::types::JsonStorage, json_storage);
                }
            }
            crate::types::JsonStorage::Null => {
                // 写入NULL值
                unsafe {
                    let json_storage = crate::types::JsonStorage::Null;
                    core::ptr::write_unaligned(field_ptr as *mut crate::types::JsonStorage, json_storage);
                }
            }
        }
        
        Ok(())
    }

    /// 设置记录数（仅用于快照恢复）
    pub unsafe fn set_record_count(&mut self, count: usize) {
        self.record_count = count;
    }

    /// 增加记录数（仅用于快照恢复）
    pub unsafe fn inc_record_count(&mut self) {
        self.record_count += 1;
    }

    /// 批量插入记录
    /// 参数：records - 指向记录数组的指针，count - 要插入的记录数，out_ids - 输出记录ID的数组指针
    /// 返回：成功插入的记录数
    pub unsafe fn batch_insert(
        &mut self,
        records: *const u8,
        count: usize,
        out_ids: *mut usize,
    ) -> Result<usize> {
        // 检查输入参数
        if records.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);

        // 计算最大记录数
        let max_records = if self.low_power_mode {
            self.low_power_max_records.unwrap_or(self.def.max_records)
        } else {
            self.def.max_records
        };

        // 检查是否有足够空间
        let available = max_records - self.record_count;
        let mut actual_count = count;

        if self.low_power_mode && self.record_count >= max_records {
            // 低功耗模式：可以覆盖旧记录
            actual_count = count;
        } else if available < count {
            // 正常模式或低功耗模式下有空间限制
            actual_count = available;
        }

        if actual_count == 0 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::OutOfMemory);
        }

        // 批量获取空闲槽
        let mut slot_ids = [0usize; 256]; // 最多一次处理256条记录
        assert!(
            actual_count <= slot_ids.len(),
            "Batch insert count exceeds maximum"
        );

        let mut inserted_count = 0;
        let mut i = 0;

        // 优先使用空闲槽
        while i < actual_count && self.free_slot_count > 0 {
            slot_ids[i] = *self.free_slots.as_ptr().add(self.free_slot_count - 1);
            self.free_slot_count -= 1;
            inserted_count += 1;
            i += 1;
        }

        // 如果空闲槽不够，在低功耗模式下覆盖旧记录
        if i < actual_count && self.low_power_mode {
            // 查找最旧的记录
            let mut oldest_ids = [0usize; 256];
            let mut oldest_versions = [u16::MAX; 256];

            for record_id in 0..self.def.max_records {
                let status_ptr = self.status_array.as_ptr().add(record_id);
                let status = &*status_ptr;
                if status.status == crate::types::RecordStatus::Used {
                    // 找到比当前最旧版本更旧的记录
                    for j in 0..(actual_count - i) {
                        if status.version < oldest_versions[j] {
                            // 插入到合适位置
                            for k in (j + 1)..(actual_count - i) {
                                if oldest_versions[k] > oldest_versions[k - 1] {
                                    break;
                                }
                                oldest_ids[k] = oldest_ids[k - 1];
                                oldest_versions[k] = oldest_versions[k - 1];
                            }
                            oldest_ids[j] = record_id;
                            oldest_versions[j] = status.version;
                            break;
                        }
                    }
                }
            }

            // 使用找到的最旧记录槽
            for j in 0..(actual_count - i) {
                slot_ids[i + j] = oldest_ids[j];
            }
            inserted_count = actual_count;
        }

        // 检查是否有活跃事务
        let has_active_tx = crate::transaction::has_active_tx();
        let current_tx = if has_active_tx {
            crate::transaction::get_current_tx()
        } else {
            None
        };

        // 释放锁，准备批量处理
        crate::platform::spin_unlock(&mut self.lock);

        // 批量处理记录，不持有锁
        for j in 0..inserted_count {
            let slot_id = slot_ids[j];

            // 保存记录ID到输出数组
            if !out_ids.is_null() {
                *out_ids.add(j) = slot_id;
            }

            // 计算记录地址
            let record_ptr = self.data_start.as_ptr().add(slot_id * self.record_size);
            let src_ptr = records.add(j * self.record_size);

            // 记录日志（如果有活跃事务）
            if let Some(mut tx) = current_tx {
                let tx_mut = tx.as_mut();
                if tx_mut.is_active() && !tx_mut.is_read_only() {
                    let mut new_data = Vec::with_capacity(self.record_size);
                    new_data.resize(self.record_size, 0);
                    memcpy(new_data.as_mut_ptr(), src_ptr, self.record_size);

                    let var_log_item = tx_mut.begin_variable_size_log_item(
                        tx_mut.id,
                        crate::transaction::LogOperation::Insert,
                        self.def.id,
                        slot_id as u16,
                        None,
                        Some(&new_data),
                    );
                    if let Some(log_manager) = crate::transaction::get_log_manager() {
                        log_manager.write_variable_size_log_item(&var_log_item).unwrap_or(());
                    }
                }
            }

            // 拷贝记录数据
            memcpy(record_ptr, src_ptr, self.record_size);

            // 更新状态
            let status_ptr = self.status_array.as_ptr().add(slot_id);
            (*status_ptr).status = crate::types::RecordStatus::Used;
            (*status_ptr).version += 1;
        }

        // 再次加锁，更新记录计数
        crate::platform::spin_lock(&mut self.lock);

        // 计算实际增加的记录数（只增加新插入的记录，不包括覆盖的记录）
        let new_records_count = if self.low_power_mode && self.record_count >= max_records {
            0 // 低功耗模式下覆盖旧记录，记录数不变
        } else {
            inserted_count // 新插入的记录数
        };

        self.record_count += new_records_count;
        crate::platform::spin_unlock(&mut self.lock);

        Ok(inserted_count)
    }

    /// 时间序列批量写入优化
    /// 参数：records - 指向记录数组的指针，count - 要插入的记录数，out_ids - 输出记录ID的数组指针
    /// 返回：成功插入的记录数
    pub unsafe fn time_series_batch_insert(
        &mut self,
        records: *const u8,
        count: usize,
        out_ids: *mut usize,
    ) -> Result<usize> {
        // 此方法针对时间序列数据的高频率写入进行优化
        // 假设数据按时间顺序写入，且不需要事务日志

        // 检查输入参数
        if records.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);

        // 检查是否有足够空间
        let available = self.def.max_records - self.record_count;
        let actual_count = core::cmp::min(count, available);
        let actual_count = core::cmp::min(actual_count, self.free_slot_count);

        if actual_count == 0 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::OutOfMemory);
        }

        // 批量获取空闲槽
        let original_free_slot_count = self.free_slot_count;
        let end_free_slot = self.free_slot_count - actual_count;

        // 批量更新空闲槽计数
        self.free_slot_count = end_free_slot;

        // 检查是否有活跃事务
        let has_active_tx = crate::transaction::has_active_tx();
        let current_tx = if has_active_tx {
            crate::transaction::get_current_tx()
        } else {
            None
        };

        // 解锁，减少锁持有时间
        crate::platform::spin_unlock(&mut self.lock);

        // 批量处理记录，不持有锁
        let mut inserted_count = 0;

        for i in 0..actual_count {
            // 从栈顶开始获取空闲槽，正确的索引是 original_free_slot_count - 1 - i
            let free_slot_index = original_free_slot_count - 1 - i;
            let slot_id = *self.free_slots.as_ptr().add(free_slot_index);

            // 保存记录ID到输出数组
            if !out_ids.is_null() {
                *out_ids.add(i) = slot_id;
            }

            // 计算记录地址
            let record_ptr = self.data_start.as_ptr().add(slot_id * self.record_size);
            let src_ptr = records.add(i * self.record_size);

            // 记录日志（如果有活跃事务）
            if let Some(mut tx) = current_tx {
                let tx_mut = tx.as_mut();
                if tx_mut.is_active() && !tx_mut.is_read_only() {
                    let mut new_data = Vec::with_capacity(self.record_size);
                    new_data.resize(self.record_size, 0);
                    memcpy(new_data.as_mut_ptr(), src_ptr, self.record_size);

                    let var_log_item = tx_mut.begin_variable_size_log_item(
                        tx_mut.id,
                        crate::transaction::LogOperation::TimeSeriesInsert,
                        self.def.id,
                        slot_id as u16,
                        None,
                        Some(&new_data),
                    );
                    if let Some(log_manager) = crate::transaction::get_log_manager() {
                        log_manager.write_variable_size_log_item(&var_log_item).unwrap_or(());
                    }
                }
            }

            // 拷贝记录数据
            memcpy(record_ptr, src_ptr, self.record_size);

            // 更新状态（简化版本，减少版本号更新频率）
            let status_ptr = self.status_array.as_ptr().add(slot_id);
            (*status_ptr).status = RecordStatus::Used;

            inserted_count += 1;
        }

        // 再次加锁，更新记录计数
        crate::platform::spin_lock(&mut self.lock);
        self.record_count += inserted_count;
        crate::platform::spin_unlock(&mut self.lock);

        Ok(inserted_count)
    }

    /// 批量获取记录
    /// 参数：ids - 要获取的记录ID数组，dest - 存储结果的缓冲区
    /// 返回：成功获取的记录数
    pub unsafe fn batch_get(&self, ids: &[usize], dest: *mut u8) -> Result<usize> {
        let mut success_count = 0;

        for (i, &id) in ids.iter().enumerate() {
            // 检查ID有效性
            if id >= self.def.max_records {
                continue;
            }

            let status_ptr = self.status_array.as_ptr().add(id);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            // 拷贝记录数据
            let record_ptr = self.data_start.as_ptr().add(id * self.record_size);
            let dest_ptr = dest.add(i * self.record_size);
            memcpy(dest_ptr, record_ptr, self.record_size);

            success_count += 1;
        }

        Ok(success_count)
    }

    /// 时间序列聚合：统计时间范围内记录数
    /// 参数：time_field_index - 时间字段索引，start_time - 开始时间，end_time - 结束时间
    pub unsafe fn aggregate_count(
        &self,
        time_field_index: usize,
        start_time: u64,
        end_time: u64,
    ) -> Result<usize> {
        // 检查时间字段索引有效性
        if time_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let mut count = 0;
        let time_field = &self.def.fields[time_field_index];

        // 遍历所有记录，统计符合时间范围的记录数
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 根据字段类型读取时间值
            let timestamp = match time_field.data_type {
                crate::types::DataType::UInt64 => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                _ => {
                    // 对于其他数值类型，先读取为i64，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => {
                            core::ptr::read_unaligned(field_ptr as *const u8) as u64
                        }
                        crate::types::DataType::UInt16 => {
                            core::ptr::read_unaligned(field_ptr as *const u16) as u64
                        }
                        crate::types::DataType::UInt32 => {
                            core::ptr::read_unaligned(field_ptr as *const u32) as u64
                        }
                        crate::types::DataType::Int8 => {
                            core::ptr::read_unaligned(field_ptr as *const i8) as u64
                        }
                        crate::types::DataType::Int16 => {
                            core::ptr::read_unaligned(field_ptr as *const i16) as u64
                        }
                        crate::types::DataType::Int32 => {
                            core::ptr::read_unaligned(field_ptr as *const i32) as u64
                        }
                        crate::types::DataType::Int64 => {
                            core::ptr::read_unaligned(field_ptr as *const i64) as u64
                        }
                        _ => continue, // 跳过非数值类型
                    }
                }
            };

            if timestamp >= start_time && timestamp <= end_time {
                count += 1;
            }
        }

        Ok(count)
    }

    /// 时间序列聚合：计算时间范围内数值字段总和
    /// 参数：time_field_index - 时间字段索引，value_field_index - 数值字段索引，start_time - 开始时间，end_time - 结束时间
    pub unsafe fn aggregate_sum(
        &self,
        time_field_index: usize,
        value_field_index: usize,
        start_time: u64,
        end_time: u64,
    ) -> Result<f64> {
        // 检查字段索引有效性
        if time_field_index >= self.def.fields.len() || value_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let mut sum = 0.0;

        // 遍历所有记录，计算符合时间范围的数值总和
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 根据字段类型读取时间值
            let time_field = &self.def.fields[time_field_index];
            let timestamp = match time_field.data_type {
                crate::types::DataType::UInt64 => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                _ => {
                    // 对于其他数值类型，先读取对应类型，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => {
                            core::ptr::read_unaligned(field_ptr as *const u8) as u64
                        }
                        crate::types::DataType::UInt16 => {
                            core::ptr::read_unaligned(field_ptr as *const u16) as u64
                        }
                        crate::types::DataType::UInt32 => {
                            core::ptr::read_unaligned(field_ptr as *const u32) as u64
                        }
                        crate::types::DataType::Int8 => {
                            core::ptr::read_unaligned(field_ptr as *const i8) as u64
                        }
                        crate::types::DataType::Int16 => {
                            core::ptr::read_unaligned(field_ptr as *const i16) as u64
                        }
                        crate::types::DataType::Int32 => {
                            core::ptr::read_unaligned(field_ptr as *const i32) as u64
                        }
                        crate::types::DataType::Int64 => {
                            core::ptr::read_unaligned(field_ptr as *const i64) as u64
                        }
                        _ => continue, // 跳过非数值类型
                    }
                }
            };

            if timestamp >= start_time && timestamp <= end_time {
                // 获取数值
                let value = self.get_field(record_ptr, value_field_index)?;
                let numeric_value = match self.def.fields[value_field_index].data_type {
                    crate::types::DataType::UInt8 => value.u8 as f64,
                    crate::types::DataType::UInt16 => value.u16 as f64,
                    crate::types::DataType::UInt32 => value.u32 as f64,
                    crate::types::DataType::UInt64 => value.u64 as f64,
                    crate::types::DataType::Float32 => value.float32 as f64,
                    crate::types::DataType::Float64 => value.float64,
                    _ => return Err(RemDbError::TypeMismatch),
                };

                sum += numeric_value;
            }
        }

        Ok(sum)
    }

    /// 时间序列聚合：计算时间范围内数值字段平均值
    /// 参数：time_field_index - 时间字段索引，value_field_index - 数值字段索引，start_time - 开始时间，end_time - 结束时间
    pub unsafe fn aggregate_avg(
        &self,
        time_field_index: usize,
        value_field_index: usize,
        start_time: u64,
        end_time: u64,
    ) -> Result<f64> {
        // 检查字段索引有效性
        if time_field_index >= self.def.fields.len() || value_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let mut sum = 0.0;
        let mut count = 0;

        // 遍历所有记录，计算符合时间范围的数值总和和计数
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 根据字段类型读取时间值
            let time_field = &self.def.fields[time_field_index];
            let timestamp = match time_field.data_type {
                crate::types::DataType::UInt64 => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                _ => {
                    // 对于其他数值类型，先读取对应类型，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => {
                            core::ptr::read_unaligned(field_ptr as *const u8) as u64
                        }
                        crate::types::DataType::UInt16 => {
                            core::ptr::read_unaligned(field_ptr as *const u16) as u64
                        }
                        crate::types::DataType::UInt32 => {
                            core::ptr::read_unaligned(field_ptr as *const u32) as u64
                        }
                        crate::types::DataType::Int8 => {
                            core::ptr::read_unaligned(field_ptr as *const i8) as u64
                        }
                        crate::types::DataType::Int16 => {
                            core::ptr::read_unaligned(field_ptr as *const i16) as u64
                        }
                        crate::types::DataType::Int32 => {
                            core::ptr::read_unaligned(field_ptr as *const i32) as u64
                        }
                        crate::types::DataType::Int64 => {
                            core::ptr::read_unaligned(field_ptr as *const i64) as u64
                        }
                        _ => continue, // 跳过非数值类型
                    }
                }
            };

            if timestamp >= start_time && timestamp <= end_time {
                // 获取数值
                let value = self.get_field(record_ptr, value_field_index)?;
                let numeric_value = match self.def.fields[value_field_index].data_type {
                    crate::types::DataType::UInt8 => value.u8 as f64,
                    crate::types::DataType::UInt16 => value.u16 as f64,
                    crate::types::DataType::UInt32 => value.u32 as f64,
                    crate::types::DataType::UInt64 => value.u64 as f64,
                    crate::types::DataType::Float32 => value.float32 as f64,
                    crate::types::DataType::Float64 => value.float64,
                    _ => return Err(RemDbError::TypeMismatch),
                };

                sum += numeric_value;
                count += 1;
            }
        }

        if count == 0 {
            Ok(0.0)
        } else {
            Ok(sum / count as f64)
        }
    }

    /// 时间序列聚合：计算时间范围内数值字段最小值
    /// 参数：time_field_index - 时间字段索引，value_field_index - 数值字段索引，start_time - 开始时间，end_time - 结束时间
    pub unsafe fn aggregate_min(
        &self,
        time_field_index: usize,
        value_field_index: usize,
        start_time: u64,
        end_time: u64,
    ) -> Result<f64> {
        // 检查字段索引有效性
        if time_field_index >= self.def.fields.len() || value_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let mut min_value: Option<f64> = None;

        // 遍历所有记录，找到符合时间范围的数值最小值
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 根据字段类型读取时间值
            let time_field = &self.def.fields[time_field_index];
            let timestamp = match time_field.data_type {
                crate::types::DataType::UInt64 => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(record_ptr.add(time_field.offset) as *const u64)
                }
                _ => {
                    // 对于其他数值类型，先读取对应类型，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => {
                            core::ptr::read_unaligned(field_ptr as *const u8) as u64
                        }
                        crate::types::DataType::UInt16 => {
                            core::ptr::read_unaligned(field_ptr as *const u16) as u64
                        }
                        crate::types::DataType::UInt32 => {
                            core::ptr::read_unaligned(field_ptr as *const u32) as u64
                        }
                        crate::types::DataType::Int8 => {
                            core::ptr::read_unaligned(field_ptr as *const i8) as u64
                        }
                        crate::types::DataType::Int16 => {
                            core::ptr::read_unaligned(field_ptr as *const i16) as u64
                        }
                        crate::types::DataType::Int32 => {
                            core::ptr::read_unaligned(field_ptr as *const i32) as u64
                        }
                        crate::types::DataType::Int64 => {
                            core::ptr::read_unaligned(field_ptr as *const i64) as u64
                        }
                        _ => continue, // 跳过非数值类型
                    }
                }
            };

            if timestamp >= start_time && timestamp <= end_time {
                // 获取数值
                let value = self.get_field(record_ptr, value_field_index)?;
                let numeric_value = match self.def.fields[value_field_index].data_type {
                    crate::types::DataType::UInt8 => value.u8 as f64,
                    crate::types::DataType::UInt16 => value.u16 as f64,
                    crate::types::DataType::UInt32 => value.u32 as f64,
                    crate::types::DataType::UInt64 => value.u64 as f64,
                    crate::types::DataType::Float32 => value.float32 as f64,
                    crate::types::DataType::Float64 => value.float64,
                    _ => return Err(RemDbError::TypeMismatch),
                };

                if let Some(current_min) = min_value {
                    if numeric_value < current_min {
                        min_value = Some(numeric_value);
                    }
                } else {
                    min_value = Some(numeric_value);
                }
            }
        }

        min_value.ok_or(RemDbError::RecordNotFound)
    }

    /// 辅助函数：根据字段类型读取时间值
    unsafe fn read_timestamp_value(
        &self,
        record_ptr: *const u8,
        time_field_index: usize,
    ) -> Option<u64> {
        let time_field = &self.def.fields[time_field_index];
        match time_field.data_type {
            crate::types::DataType::UInt64 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const u64,
            )),
            crate::types::DataType::Timestamp => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const u64,
            )),
            crate::types::DataType::UInt8 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const u8,
            ) as u64),
            crate::types::DataType::UInt16 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const u16,
            ) as u64),
            crate::types::DataType::UInt32 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const u32,
            ) as u64),
            crate::types::DataType::Int8 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const i8,
            ) as u64),
            crate::types::DataType::Int16 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const i16,
            ) as u64),
            crate::types::DataType::Int32 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const i32,
            ) as u64),
            crate::types::DataType::Int64 => Some(core::ptr::read_unaligned(
                record_ptr.add(time_field.offset) as *const i64,
            ) as u64),
            _ => None, // 跳过非数值类型
        }
    }

    /// 时间序列聚合：计算时间范围内数值字段最大值
    /// 参数：time_field_index - 时间字段索引，value_field_index - 数值字段索引，start_time - 开始时间，end_time - 结束时间
    pub unsafe fn aggregate_max(
        &self,
        time_field_index: usize,
        value_field_index: usize,
        start_time: u64,
        end_time: u64,
    ) -> Result<f64> {
        // 检查字段索引有效性
        if time_field_index >= self.def.fields.len() || value_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let mut max_value: Option<f64> = None;

        // 遍历所有记录，找到符合时间范围的数值最大值
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 使用辅助函数读取时间值
            let Some(timestamp) = self.read_timestamp_value(record_ptr, time_field_index) else {
                continue;
            };

            if timestamp >= start_time && timestamp <= end_time {
                // 获取数值
                let value = self.get_field(record_ptr, value_field_index)?;
                let numeric_value = match self.def.fields[value_field_index].data_type {
                    crate::types::DataType::UInt8 => value.u8 as f64,
                    crate::types::DataType::UInt16 => value.u16 as f64,
                    crate::types::DataType::UInt32 => value.u32 as f64,
                    crate::types::DataType::UInt64 => value.u64 as f64,
                    crate::types::DataType::Float32 => value.float32 as f64,
                    crate::types::DataType::Float64 => value.float64,
                    _ => return Err(RemDbError::TypeMismatch),
                };

                if let Some(current_max) = max_value {
                    if numeric_value > current_max {
                        max_value = Some(numeric_value);
                    }
                } else {
                    max_value = Some(numeric_value);
                }
            }
        }

        max_value.ok_or(RemDbError::RecordNotFound)
    }

    /// 获取最新记录
    /// 参数：time_field_index - 时间字段索引，count - 要获取的记录数，dest - 存储结果的缓冲区
    pub unsafe fn get_latest_records(
        &self,
        time_field_index: usize,
        count: usize,
        dest: *mut u8,
    ) -> Result<usize> {
        // 检查时间字段索引有效性
        if time_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        // 检查输出缓冲区是否为null
        if dest.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 如果没有记录，直接返回
        if self.record_count == 0 {
            return Ok(0);
        }

        // 创建一个固定大小的数组来存储记录ID和时间值
        // 使用最大记录数作为数组大小
        let mut record_times = [(0usize, 0u64); 1024]; // 假设最大记录数不超过1024
        let mut record_count = 0;

        // 遍历所有记录，收集记录ID和时间值
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 使用辅助函数读取时间值
            if let Some(timestamp) = self.read_timestamp_value(record_ptr, time_field_index) {
                record_times[record_count] = (i, timestamp);
                record_count += 1;
            };
        }

        // 按时间值降序排序
        // 使用冒泡排序，避免依赖标准库的sort方法
        for i in 0..record_count {
            for j in i + 1..record_count {
                if record_times[i].1 < record_times[j].1 {
                    record_times.swap(i, j);
                }
            }
        }

        // 拷贝最新的count条记录到输出缓冲区
        let actual_count = core::cmp::min(count, record_count);
        for i in 0..actual_count {
            let (record_id, _) = record_times[i];
            let src_ptr = self.data_start.as_ptr().add(record_id * self.record_size);
            let dest_ptr = dest.add(i * self.record_size);
            memcpy(dest_ptr, src_ptr, self.record_size);
        }

        Ok(actual_count)
    }

    /// 获取时间窗口内的记录
    /// 参数：time_field_index - 时间字段索引，start_time - 开始时间，end_time - 结束时间，dest - 存储结果的缓冲区，max_records - 最大返回记录数
    pub unsafe fn get_records_in_time_window(
        &self,
        time_field_index: usize,
        start_time: u64,
        end_time: u64,
        dest: *mut u8,
        max_records: usize,
    ) -> Result<usize> {
        // 检查时间字段索引有效性
        if time_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        // 检查输出缓冲区是否为null
        if dest.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 如果没有记录，直接返回
        if self.record_count == 0 {
            return Ok(0);
        }

        // 创建一个固定大小的数组来存储符合时间范围的记录ID和时间值
        let mut matched_records = [(0usize, 0u64); 1024]; // 假设最大记录数不超过1024
        let mut match_count = 0;

        // 遍历所有记录，收集符合时间范围的记录
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 使用辅助函数读取时间值
            if let Some(timestamp) = self.read_timestamp_value(record_ptr, time_field_index) {
                if timestamp >= start_time && timestamp <= end_time {
                    matched_records[match_count] = (i, timestamp);
                    match_count += 1;
                }
            };
        }

        // 按时间值升序排序
        // 使用冒泡排序，避免依赖标准库的sort方法
        for i in 0..match_count {
            for j in i + 1..match_count {
                if matched_records[i].1 > matched_records[j].1 {
                    matched_records.swap(i, j);
                }
            }
        }

        // 拷贝符合条件的记录到输出缓冲区
        let actual_count = core::cmp::min(max_records, match_count);
        for i in 0..actual_count {
            let (record_id, _) = matched_records[i];
            let src_ptr = self.data_start.as_ptr().add(record_id * self.record_size);
            let dest_ptr = dest.add(i * self.record_size);
            memcpy(dest_ptr, src_ptr, self.record_size);
        }

        Ok(actual_count)
    }

    /// 按时间窗口聚合
    /// 参数：time_field_index - 时间字段索引，value_field_index - 数值字段索引，start_time - 开始时间，end_time - 结束时间，window_size - 窗口大小（毫秒）
    #[cfg(feature = "std")]
    pub unsafe fn get_aggregate_in_time_window(
        &self,
        time_field_index: usize,
        value_field_index: usize,
        start_time: u64,
        end_time: u64,
        window_size: u64,
    ) -> Result<Vec<(u64, f64, f64, f64, f64, usize)>> {
        // 检查字段索引有效性
        if time_field_index >= self.def.fields.len() || value_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        let time_field = &self.def.fields[time_field_index];
        let value_field = &self.def.fields[value_field_index];

        // 检查时间字段类型
        if time_field.data_type != crate::types::DataType::Timestamp
            && time_field.data_type != crate::types::DataType::UInt64
        {
            return Err(RemDbError::TypeMismatch);
        }

        // 创建一个HashMap来存储每个时间窗口的聚合数据
        use alloc::collections::BTreeMap;
        let mut window_aggregates: BTreeMap<u64, (f64, f64, f64, f64, usize)> = BTreeMap::new();

        // 遍历所有记录，按时间窗口聚合
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status != RecordStatus::Used {
                continue;
            }

            let record_ptr = self.data_start.as_ptr().add(i * self.record_size);

            // 使用辅助函数读取时间值
            let Some(time_value) = self.read_timestamp_value(record_ptr, time_field_index) else {
                continue;
            };

            if time_value >= start_time && time_value <= end_time {
                // 获取数值
                let value = self.get_field(record_ptr, value_field_index)?;
                let numeric_value = match value_field.data_type {
                    crate::types::DataType::UInt8 => value.u8 as f64,
                    crate::types::DataType::UInt16 => value.u16 as f64,
                    crate::types::DataType::UInt32 => value.u32 as f64,
                    crate::types::DataType::UInt64 => value.u64 as f64,
                    crate::types::DataType::Float32 => value.float32 as f64,
                    crate::types::DataType::Float64 => value.float64,
                    _ => return Err(RemDbError::TypeMismatch),
                };

                // 计算时间窗口键
                let window_key = time_value - (time_value % window_size);

                // 更新聚合数据
                let entry = window_aggregates.entry(window_key).or_insert((
                    0.0,
                    numeric_value,
                    numeric_value,
                    0.0,
                    0,
                ));
                entry.0 += numeric_value; // sum
                if numeric_value < entry.1 {
                    entry.1 = numeric_value;
                } // min
                if numeric_value > entry.2 {
                    entry.2 = numeric_value;
                } // max
                entry.3 = numeric_value; // last
                entry.4 += 1; // count
            }
        }

        // 将聚合结果转换为向量
        let mut result = Vec::with_capacity(window_aggregates.len());
        for (window_start, (sum, min, max, _last, count)) in window_aggregates {
            let avg = if count > 0 { sum / count as f64 } else { 0.0 };
            result.push((window_start, sum, avg, min, max, count));
        }

        Ok(result)
    }

    /// no_std环境下不支持的聚合函数
    #[cfg(not(feature = "std"))]
    pub unsafe fn get_aggregate_in_time_window(
        &self,
        _time_field_index: usize,
        _value_field_index: usize,
        _start_time: u64,
        _end_time: u64,
        _window_size: u64,
    ) -> Result<()> {
        // no_std环境下不支持Vec和BTreeMap，返回错误
        Err(RemDbError::UnsupportedOperation)
    }
}

/// 延迟释放锁的宏
#[macro_export]
macro_rules! defer {
    ($($code:tt)*) => {
        let _defer = $crate::table::Defer::new(|| { $($code)* });
    };
}

/// 延迟执行结构体
pub struct Defer<F: FnMut()>(Option<F>);

impl<F: FnMut()> Defer<F> {
    /// 创建新的延迟执行实例
    pub fn new(f: F) -> Self {
        Defer(Some(f))
    }
}

impl<F: FnMut()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(mut f) = self.0.take() {
            f();
        }
    }
}
