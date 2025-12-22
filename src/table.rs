use core::ptr::NonNull;
use crate::types::{RecordHeader, RecordStatus, TableDef, Value, Result, RemDbError};
use crate::platform::{memcpy, memset};
use crate::defer;

/// 内存表
pub struct MemoryTable {
    /// 表定义
    pub def: &'static TableDef,
    /// 表数据起始地址
    data_start: NonNull<u8>,
    /// 记录状态数组
    status_array: NonNull<RecordHeader>,
    /// 当前记录数
    record_count: usize,
    /// 自旋锁
    lock: u32,
}

impl MemoryTable {
    /// 创建新的内存表
    pub unsafe fn new(
        def: &'static TableDef,
        data_start: *mut u8,
        status_start: *mut RecordHeader
    ) -> Self {
        // 初始化状态数组
        let status_array = NonNull::new_unchecked(status_start);
        for i in 0..def.max_records {
            let status_ptr = status_array.as_ptr().add(i);
            (*status_ptr).status = RecordStatus::Free;
            (*status_ptr).version = 0;
        }
        
        MemoryTable {
            def,
            data_start: NonNull::new_unchecked(data_start),
            status_array,
            record_count: 0,
            lock: 0,
        }
    }
    
    /// 计算表所需的总内存大小
    pub const fn calculate_memory_size(def: &'static TableDef) -> usize {
        // 数据大小：记录大小 * 最大记录数
        let data_size = def.record_size * def.max_records;
        // 状态数组大小：RecordHeader大小 * 最大记录数
        let status_size = core::mem::size_of::<RecordHeader>() * def.max_records;
        
        data_size + status_size
    }
    
    /// 插入记录
    pub unsafe fn insert(&mut self, record_data: *const u8) -> Result<usize> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        // 检查是否已满
        if self.record_count >= self.def.max_records {
            return Err(RemDbError::OutOfMemory);
        }
        
        // 查找空闲记录槽
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status == RecordStatus::Free {
                // 计算记录地址
                let record_ptr = self.data_start.as_ptr().add(i * self.def.record_size);
                
                // 拷贝记录数据
                memcpy(record_ptr, record_data, self.def.record_size);
                
                // 更新状态
                (*status_ptr).status = RecordStatus::Used;
                (*status_ptr).version += 1;
                
                // 更新记录计数
                self.record_count += 1;
                
                return Ok(i);
            }
        }
        
        Err(RemDbError::OutOfMemory)
    }
    
    /// 删除记录
    pub unsafe fn delete(&mut self, id: usize) -> Result<()> {
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
        
        // 标记为已删除
        (*status_ptr).status = RecordStatus::Deleted;
        (*status_ptr).version += 1;
        
        // 清空记录数据
        let record_ptr = self.data_start.as_ptr().add(id * self.def.record_size);
        memset(record_ptr, 0, self.def.record_size);
        
        // 更新记录计数
        self.record_count -= 1;
        
        Ok(())
    }
    
    /// 根据ID获取记录
    pub unsafe fn get_by_id(&self, id: usize, dest: *mut u8) -> Result<()> {
        // 检查ID有效性
        if id >= self.def.max_records {
            return Err(RemDbError::RecordNotFound);
        }
        
        let status_ptr = self.status_array.as_ptr().add(id);
        if (*status_ptr).status != RecordStatus::Used {
            return Err(RemDbError::RecordNotFound);
        }
        
        // 拷贝记录数据
        let record_ptr = self.data_start.as_ptr().add(id * self.def.record_size);
        memcpy(dest, record_ptr, self.def.record_size);
        
        Ok(())
    }
    
    /// 获取字段值
    pub unsafe fn get_field(
        &self,
        record_data: *const u8,
        field_index: usize
    ) -> Result<Value> {
        // 检查字段索引有效性
        if field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }
        
        let field = &self.def.fields[field_index];
        let field_ptr = record_data.add(field.offset);
        
        // 根据字段类型获取值
        let value = match field.data_type {
            crate::types::DataType::Int8 => {
                Value { int8: *field_ptr as i8 }
            }
            crate::types::DataType::Int16 => {
                Value { int16: *(field_ptr as *const i16) }
            }
            crate::types::DataType::Int32 => {
                Value { int32: *(field_ptr as *const i32) }
            }
            crate::types::DataType::Int64 => {
                Value { int64: *(field_ptr as *const i64) }
            }
            crate::types::DataType::Float32 => {
                Value { float32: *(field_ptr as *const f32) }
            }
            crate::types::DataType::Float64 => {
                Value { float64: *(field_ptr as *const f64) }
            }
            crate::types::DataType::Bool => {
                Value { bool: *field_ptr != 0 }
            }
            crate::types::DataType::Timestamp => {
                Value { timestamp: *(field_ptr as *const u64) }
            }
            crate::types::DataType::String => {
                let mut str_value = [0u8; crate::types::MAX_STRING_LEN];
                memcpy(str_value.as_mut_ptr(), field_ptr, field.size);
                Value { string: str_value }
            }
        };
        
        Ok(value)
    }
    
    /// 设置字段值
    pub unsafe fn set_field(
        &self,
        record_data: *mut u8,
        field_index: usize,
        value: &Value
    ) -> Result<()> {
        // 检查字段索引有效性
        if field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }
        
        let field = &self.def.fields[field_index];
        let field_ptr = record_data.add(field.offset);
        
        // 根据字段类型设置值
        match field.data_type {
            crate::types::DataType::Int8 => {
                *(field_ptr as *mut i8) = value.int8;
            }
            crate::types::DataType::Int16 => {
                *(field_ptr as *mut i16) = value.int16;
            }
            crate::types::DataType::Int32 => {
                *(field_ptr as *mut i32) = value.int32;
            }
            crate::types::DataType::Int64 => {
                *(field_ptr as *mut i64) = value.int64;
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
                *(field_ptr as *mut u64) = value.timestamp;
            }
            crate::types::DataType::String => {
                memcpy(field_ptr, value.string.as_ptr(), field.size);
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
    
    /// 检查表是否为空
    pub fn is_empty(&self) -> bool {
        self.record_count == 0
    }
    
    /// 遍历记录
    pub unsafe fn iterate<F>(&self, mut f: F) -> Result<()>
    where F: FnMut(usize, *const u8) -> bool {
        for i in 0..self.def.max_records {
            let status_ptr = self.status_array.as_ptr().add(i);
            if (*status_ptr).status == RecordStatus::Used {
                let record_ptr = self.data_start.as_ptr().add(i * self.def.record_size);
                if !f(i, record_ptr) {
                    break;
                }
            }
        }
        
        Ok(())
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
