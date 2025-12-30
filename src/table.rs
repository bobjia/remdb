use core::ptr::NonNull;
use crate::{types::{RecordHeader, RecordStatus, TableDef, Value, Result, RemDbError, DataType}, DataType as CrateDataType};
use crate::platform::{memcpy, memset};
use crate::defer;

// 引入alloc模块
extern crate alloc;

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
    /// 下一个自增ID值
    pub next_auto_id: u64,
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
        // 计算所需内存大小
        let data_size = def.record_size * def.max_records;
        let status_size = core::mem::size_of::<RecordHeader>() * def.max_records;
        let free_slots_size = core::mem::size_of::<usize>() * def.max_records;
        
        // 动态分配内存
        let data_start = crate::memory::allocator::alloc(data_size)?;
        let status_start = crate::memory::allocator::alloc(status_size)?;
        let free_slots_start = crate::memory::allocator::alloc(free_slots_size)?;
        
        // 初始化状态数组
        unsafe {
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
        }
        
        Ok(MemoryTable {
            def: def.clone(),
            data_start,
            status_array: status_start.cast(),
            record_count: 0,
            lock: 0,
            record_size: def.record_size, // 使用表定义中已经计算好的record_size
            free_slots: free_slots_start.cast(),
            free_slot_count: def.max_records,
            low_power_mode: false, // 默认不启用低功耗模式
            low_power_max_records: None, // 默认使用表定义的最大记录数
            snapshot_version: 0, // 初始快照版本为0
            next_auto_id: 1, // 自增ID从1开始
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
    pub unsafe fn validate_constraints(&self, record_data: *const u8, exclude_slot: Option<usize>) -> Result<()>
    {
        // 验证非空约束
        // 注意：在当前实现中，RemDB没有真正的NULL支持机制
        // not null constraint主要是防止用户插入未初始化的内存
        for field in self.def.fields {
            if field.not_null {
                // 检查字段是否为空
                let is_null = match field.data_type {
                    DataType::String => {
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
                    },
                    DataType::Bool => {
                        // 布尔类型：0表示false，1表示true，两者都是有效值，所以永远不为null
                        false
                    },
                    DataType::Float32 => {
                        // 对于浮点数，检查是否是NaN（不是一个数），NaN表示无效值
                        let float_value = core::ptr::read_unaligned(record_data.add(field.offset) as *const f32);
                        float_value.is_nan()
                    },
                    DataType::Float64 => {
                        // 对于浮点数，检查是否是NaN（不是一个数），NaN表示无效值
                        let float_value = core::ptr::read_unaligned(record_data.add(field.offset) as *const f64);
                        float_value.is_nan()
                    },
                    _ => {
                        // 对于整数类型，0是一个合法的值，所以永远不为null
                        // 这里不再检查全0，因为0是合法值
                        false
                    },
                };
                if is_null {
                    return Err(RemDbError::TypeMismatch);
                }
            }
        }
        
        // 验证主键唯一性约束
        let primary_key_index = self.def.primary_key;
        if primary_key_index < self.def.fields.len() {
            // 获取主键字段定义
            let primary_key_field = &self.def.fields[primary_key_index];
            let primary_key_offset = primary_key_field.offset;
            let primary_key_data_type = primary_key_field.data_type;
            
            // 直接获取当前记录的主键指针
            let primary_key_ptr = record_data.add(primary_key_offset);
            
            // 遍历记录，直接比较内存中的主键值
            let mut has_duplicate = false;
            
            // 遍历所有记录槽，检查已使用的记录
            // 优化：只遍历已使用的记录，通过status_array检查
            // 但由于我们无法直接知道哪些槽被使用，只能遍历所有槽
            // 不过我们可以在找到重复后立即中断
            for i in 0..self.def.max_records {
                let status_ptr = self.status_array.as_ptr().add(i);
                if (*status_ptr).status == RecordStatus::Used {
                    // 跳过当前记录（如果是更新操作）
                    if Some(i) == exclude_slot {
                        continue;
                    }
                    
                    // 获取其他记录的主键指针
                    let other_record_ptr = self.data_start.as_ptr().add(i * self.record_size);
                    let other_pk_ptr = other_record_ptr.add(primary_key_offset);
                    
                    // 根据主键类型直接比较内存值
                    let is_duplicate = match primary_key_data_type {
                        DataType::UInt8 => {
                            *primary_key_ptr as u8 == *other_pk_ptr as u8
                        },
                        DataType::UInt16 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const u16) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const u16)
                        },
                        DataType::UInt32 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const u32) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const u32)
                        },
                        DataType::UInt64 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const u64) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const u64)
                        },
                        DataType::Int8 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const i8) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const i8)
                        },
                        DataType::Int16 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const i16) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const i16)
                        },
                        DataType::Int32 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const i32) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const i32)
                        },
                        DataType::Int64 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const i64) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const i64)
                        },
                        DataType::Float32 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const f32) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const f32)
                        },
                        DataType::Float64 => {
                            core::ptr::read_unaligned(primary_key_ptr as *const f64) == 
                            core::ptr::read_unaligned(other_pk_ptr as *const f64)
                        },
                        _ => {
                            // 其他类型暂时不支持主键
                            false
                        },
                    };
                    
                    if is_duplicate {
                        has_duplicate = true;
                        break;
                    }
                }
            }
            
            if has_duplicate {
                return Err(RemDbError::DuplicateKey);
            }
        }
        
        Ok(())
    }
    
    /// 获取字段值的辅助方法（按偏移量）
    unsafe fn get_field_by_offset(&self, record_data: *const u8, offset: usize, data_type: DataType, size: usize) -> Result<Value>
    {
        let field_ptr = record_data.add(offset);
        
        let value = match data_type {
            DataType::UInt8 => Value { u8: *field_ptr as u8 },
            DataType::UInt16 => Value { u16: core::ptr::read_unaligned(field_ptr as *const u16) },
            DataType::UInt32 => Value { u32: core::ptr::read_unaligned(field_ptr as *const u32) },
            DataType::UInt64 => Value { u64: core::ptr::read_unaligned(field_ptr as *const u64) },
            DataType::Int8 => Value { i8: core::ptr::read_unaligned(field_ptr as *const i8) },
            DataType::Int16 => Value { i16: core::ptr::read_unaligned(field_ptr as *const i16) },
            DataType::Int32 => Value { i32: core::ptr::read_unaligned(field_ptr as *const i32) },
            DataType::Int64 => Value { i64: core::ptr::read_unaligned(field_ptr as *const i64) },
            DataType::Float32 => Value { float32: core::ptr::read_unaligned(field_ptr as *const f32) },
            DataType::Float64 => Value { float64: core::ptr::read_unaligned(field_ptr as *const f64) },
            DataType::Bool => Value { bool: *field_ptr != 0 },
            DataType::Timestamp => Value { timestamp: core::ptr::read_unaligned(field_ptr as *const u64) },
            DataType::String => {
                let mut str_value = [0u8; crate::types::MAX_STRING_LEN];
                memcpy(str_value.as_mut_ptr(), field_ptr, size);
                Value { string: str_value }
            },
        };
        
        Ok(value)
    }
    
    /// 插入记录
    pub fn insert(&mut self, record_data: *const u8) -> Result<usize> {
        // 增加写入操作计数
        crate::get_global_db().map(|db| db.metrics.inc_write_ops());
        
        // 检查是否已满
        let max_records = if self.low_power_mode {
            self.low_power_max_records.unwrap_or(self.def.max_records)
        } else {
            self.def.max_records
        };
        
        // 处理自增主键
        let mut record_buffer = [0u8; 512];
        let record_ptr: *const u8;
        
        // 检查是否需要生成自增ID
        let primary_key_field = &self.def.fields[self.def.primary_key];
        let mut needs_auto_increment = primary_key_field.auto_increment;
        
        // 如果显式指定了主键值，则不自动生成
        if needs_auto_increment {
            unsafe {
                let pk_offset = primary_key_field.offset;
                let is_zero = match primary_key_field.data_type {
                    DataType::UInt8 => *record_data.add(pk_offset) == 0,
                    DataType::UInt16 => core::ptr::read_unaligned(record_data.add(pk_offset) as *const u16) == 0,
                    DataType::UInt32 => core::ptr::read_unaligned(record_data.add(pk_offset) as *const u32) == 0,
                    DataType::UInt64 => core::ptr::read_unaligned(record_data.add(pk_offset) as *const u64) == 0,
                    DataType::Int8 => core::ptr::read_unaligned(record_data.add(pk_offset) as *const i8) == 0,
                    DataType::Int16 => core::ptr::read_unaligned(record_data.add(pk_offset) as *const i16) == 0,
                    DataType::Int32 => core::ptr::read_unaligned(record_data.add(pk_offset) as *const i32) == 0,
                    DataType::Int64 => core::ptr::read_unaligned(record_data.add(pk_offset) as *const i64) == 0,
                    _ => true,
                };
                
                // 如果主键值不为0，则认为是显式指定的，不需要自动生成
                if !is_zero {
                    needs_auto_increment = false;
                }
            }
        }
        
        if needs_auto_increment {
            // 自旋锁保护，生成自增ID
            crate::platform::spin_lock(&mut self.lock);
            let auto_id = self.next_auto_id;
            self.next_auto_id += 1;
            crate::platform::spin_unlock(&mut self.lock);
            
            // 复制原始记录数据到缓冲区
            unsafe {
                memcpy(record_buffer.as_mut_ptr(), record_data, self.record_size);
            }
            
            // 设置自增ID
            unsafe {
                let pk_offset = primary_key_field.offset;
                match primary_key_field.data_type {
                    DataType::UInt8 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut u8) = auto_id as u8;
                    },
                    DataType::UInt16 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut u16) = auto_id as u16;
                    },
                    DataType::UInt32 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut u32) = auto_id as u32;
                    },
                    DataType::UInt64 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut u64) = auto_id;
                    },
                    DataType::Int8 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut i8) = auto_id as i8;
                    },
                    DataType::Int16 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut i16) = auto_id as i16;
                    },
                    DataType::Int32 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut i32) = auto_id as i32;
                    },
                    DataType::Int64 => {
                        *(record_buffer.as_mut_ptr().add(pk_offset) as *mut i64) = auto_id as i64;
                    },
                    _ => {
                        return Err(RemDbError::TypeMismatch);
                    }
                }
            }
            
            record_ptr = record_buffer.as_ptr();
        } else {
            record_ptr = record_data;
        }
        
        // 验证约束
        // 优化：将约束验证放在锁外，减少锁持有时间
        unsafe {
            self.validate_constraints(record_ptr, None)?;
        }
        
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        let mut slot_id = 0;
        let mut is_overwrite = false;
        
        if self.record_count >= max_records {
            if self.low_power_mode {
            // 低功耗模式：覆盖最旧的记录
            // 查找最旧的记录
            let mut oldest_id = 0;
            let mut oldest_version = u16::MAX;
            
            for i in 0..self.def.max_records {
                unsafe {
                    let status_ptr = self.status_array.as_ptr().add(i);
                    let status = &*status_ptr;
                    if status.status == RecordStatus::Used && status.version < oldest_version {
                        oldest_id = i;
                        oldest_version = status.version;
                    }
                }
            }
            
            slot_id = oldest_id;
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
        let dest_record_ptr = unsafe { self.data_start.as_ptr().add(slot_id * self.record_size) };
        
        // 记录日志（如果有活跃事务）
        if let Some(mut tx) = crate::transaction::get_current_tx() {
            let tx_mut = unsafe { tx.as_mut() };
            if tx_mut.is_active() && !tx_mut.is_read_only() {
                // 保存新数据
                let mut new_data = [0u8; 512];
                memcpy(new_data.as_mut_ptr(), record_ptr, self.record_size);
                
                // 添加日志项
                unsafe {
                    tx_mut.add_log_item(
                        crate::transaction::LogOperation::Insert,
                        self.def.id,
                        slot_id as u16,
                        core::ptr::null(),
                        new_data.as_ptr(),
                        self.record_size
                    )?;
                }
            }
        }
        
        // 拷贝记录数据
        memcpy(dest_record_ptr, record_ptr, self.record_size);
        
        // 更新状态
        let status_ptr = unsafe { self.status_array.as_ptr().add(slot_id) };
        unsafe {
            (*status_ptr).status = RecordStatus::Used;
            (*status_ptr).version += 1;
        }
        
        // 更新记录计数（如果是覆盖旧记录，不需要增加计数）
        if !is_overwrite {
            self.record_count += 1;
            // 更新内存使用：增加一条记录的内存
            crate::get_global_db().map(|db| db.metrics.add_used_memory(self.record_size));
        }
        
        Ok(slot_id)
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
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        // 计算记录地址
        let record_ptr = self.data_start.as_ptr().add(id * self.record_size);
        
        // 记录日志（如果有活跃事务）
        if let Some(mut tx) = crate::transaction::get_current_tx() {
            let tx_mut = tx.as_mut();
            if tx_mut.is_active() && !tx_mut.is_read_only() {
                // 保存旧数据
                let mut old_data = [0u8; 512];
                memcpy(old_data.as_mut_ptr(), record_ptr, self.record_size);
                
                // 保存新数据
                let mut new_data = [0u8; 512];
                memcpy(new_data.as_mut_ptr(), record_data, self.record_size);
                
                // 添加日志项
                tx_mut.add_log_item(
                    crate::transaction::LogOperation::Update,
                    self.def.id,
                    id as u16,
                    old_data.as_ptr(),
                    new_data.as_ptr(),
                    self.record_size
                )?;
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
        
        // 记录日志（如果有活跃事务）
        if let Some(mut tx) = crate::transaction::get_current_tx() {
            let tx_mut = tx.as_mut();
            if tx_mut.is_active() && !tx_mut.is_read_only() {
                // 保存旧数据
                let record_ptr = self.data_start.as_ptr().add(id * self.record_size);
                let mut old_data = [0u8; 512];
                memcpy(old_data.as_mut_ptr(), record_ptr, self.record_size);
                
                // 添加日志项
                tx_mut.add_log_item(
                    crate::transaction::LogOperation::Delete,
                    self.def.id,
                    id as u16,
                    old_data.as_ptr(),
                    core::ptr::null(),
                    self.record_size
                )?;
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
            crate::types::DataType::UInt8 => {
                Value { u8: *field_ptr as u8 }
            }
            crate::types::DataType::UInt16 => {
                Value { u16: core::ptr::read_unaligned(field_ptr as *const u16) }
            }
            crate::types::DataType::UInt32 => {
                Value { u32: core::ptr::read_unaligned(field_ptr as *const u32) }
            }
            crate::types::DataType::UInt64 => {
                Value { u64: core::ptr::read_unaligned(field_ptr as *const u64) }
            }
            crate::types::DataType::Int8 => {
                Value { i8: core::ptr::read_unaligned(field_ptr as *const i8) }
            }
            crate::types::DataType::Int16 => {
                Value { i16: core::ptr::read_unaligned(field_ptr as *const i16) }
            }
            crate::types::DataType::Int32 => {
                Value { i32: core::ptr::read_unaligned(field_ptr as *const i32) }
            }
            crate::types::DataType::Int64 => {
                Value { i64: core::ptr::read_unaligned(field_ptr as *const i64) }
            }
            crate::types::DataType::Float32 => {
                Value { float32: core::ptr::read_unaligned(field_ptr as *const f32) }
            }
            crate::types::DataType::Float64 => {
                Value { float64: core::ptr::read_unaligned(field_ptr as *const f64) }
            }
            crate::types::DataType::Bool => {
                Value { bool: *field_ptr != 0 }
            }
            crate::types::DataType::Timestamp => {
                Value { timestamp: core::ptr::read_unaligned(field_ptr as *const u64) }
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
    
    /// 设置低功耗模式
    pub fn set_low_power_mode(&mut self, enabled: bool, max_records: Option<usize>) {
        self.low_power_mode = enabled;
        self.low_power_max_records = max_records;
    }
    
    /// 检查是否处于低功耗模式
    pub fn is_low_power_mode(&self) -> bool {
        self.low_power_mode
    }
    
    /// 遍历记录
    pub unsafe fn iterate<F>(&self, mut f: F) -> Result<()>
    where F: FnMut(usize, *const u8) -> bool {
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
    pub unsafe fn get_status_ptr(&self, index: usize) -> *mut RecordHeader {
        self.status_array.as_ptr().add(index)
    }
    
    /// 获取记录数据指针
    pub unsafe fn get_record_ptr(&self, index: usize) -> *const u8 {
        self.data_start.as_ptr().add(index * self.record_size)
    }
    
    /// 获取记录数据可变指针
    pub unsafe fn get_record_ptr_mut(&mut self, index: usize) -> *mut u8 {
        self.data_start.as_ptr().add(index * self.record_size) as *mut u8
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
    pub unsafe fn batch_insert(&mut self, records: *const u8, count: usize, out_ids: *mut usize) -> Result<usize> {
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
        assert!(actual_count <= slot_ids.len(), "Batch insert count exceeds maximum");
        
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
                            for k in (j+1)..(actual_count - i) {
                                if oldest_versions[k] > oldest_versions[k-1] {
                                    break;
                                }
                                oldest_ids[k] = oldest_ids[k-1];
                                oldest_versions[k] = oldest_versions[k-1];
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
        
        // 更新空闲槽计数
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
    pub unsafe fn time_series_batch_insert(&mut self, records: *const u8, count: usize, out_ids: *mut usize) -> Result<usize> {
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
        end_time: u64
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
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                _ => {
                    // 对于其他数值类型，先读取为i64，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => core::ptr::read_unaligned(field_ptr as *const u8) as u64,
                        crate::types::DataType::UInt16 => core::ptr::read_unaligned(field_ptr as *const u16) as u64,
                        crate::types::DataType::UInt32 => core::ptr::read_unaligned(field_ptr as *const u32) as u64,
                        crate::types::DataType::Int8 => core::ptr::read_unaligned(field_ptr as *const i8) as u64,
                        crate::types::DataType::Int16 => core::ptr::read_unaligned(field_ptr as *const i16) as u64,
                        crate::types::DataType::Int32 => core::ptr::read_unaligned(field_ptr as *const i32) as u64,
                        crate::types::DataType::Int64 => core::ptr::read_unaligned(field_ptr as *const i64) as u64,
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
        end_time: u64
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
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                _ => {
                    // 对于其他数值类型，先读取对应类型，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => core::ptr::read_unaligned(field_ptr as *const u8) as u64,
                        crate::types::DataType::UInt16 => core::ptr::read_unaligned(field_ptr as *const u16) as u64,
                        crate::types::DataType::UInt32 => core::ptr::read_unaligned(field_ptr as *const u32) as u64,
                        crate::types::DataType::Int8 => core::ptr::read_unaligned(field_ptr as *const i8) as u64,
                        crate::types::DataType::Int16 => core::ptr::read_unaligned(field_ptr as *const i16) as u64,
                        crate::types::DataType::Int32 => core::ptr::read_unaligned(field_ptr as *const i32) as u64,
                        crate::types::DataType::Int64 => core::ptr::read_unaligned(field_ptr as *const i64) as u64,
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
        end_time: u64
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
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                _ => {
                    // 对于其他数值类型，先读取对应类型，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => core::ptr::read_unaligned(field_ptr as *const u8) as u64,
                        crate::types::DataType::UInt16 => core::ptr::read_unaligned(field_ptr as *const u16) as u64,
                        crate::types::DataType::UInt32 => core::ptr::read_unaligned(field_ptr as *const u32) as u64,
                        crate::types::DataType::Int8 => core::ptr::read_unaligned(field_ptr as *const i8) as u64,
                        crate::types::DataType::Int16 => core::ptr::read_unaligned(field_ptr as *const i16) as u64,
                        crate::types::DataType::Int32 => core::ptr::read_unaligned(field_ptr as *const i32) as u64,
                        crate::types::DataType::Int64 => core::ptr::read_unaligned(field_ptr as *const i64) as u64,
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
        end_time: u64
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
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                crate::types::DataType::Timestamp => {
                    core::ptr::read_unaligned(
                        record_ptr.add(time_field.offset) as *const u64
                    )
                },
                _ => {
                    // 对于其他数值类型，先读取对应类型，再转换为u64
                    let field_ptr = record_ptr.add(time_field.offset);
                    match time_field.data_type {
                        crate::types::DataType::UInt8 => core::ptr::read_unaligned(field_ptr as *const u8) as u64,
                        crate::types::DataType::UInt16 => core::ptr::read_unaligned(field_ptr as *const u16) as u64,
                        crate::types::DataType::UInt32 => core::ptr::read_unaligned(field_ptr as *const u32) as u64,
                        crate::types::DataType::Int8 => core::ptr::read_unaligned(field_ptr as *const i8) as u64,
                        crate::types::DataType::Int16 => core::ptr::read_unaligned(field_ptr as *const i16) as u64,
                        crate::types::DataType::Int32 => core::ptr::read_unaligned(field_ptr as *const i32) as u64,
                        crate::types::DataType::Int64 => core::ptr::read_unaligned(field_ptr as *const i64) as u64,
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
    unsafe fn read_timestamp_value(&self, record_ptr: *const u8, time_field_index: usize) -> Option<u64> {
        let time_field = &self.def.fields[time_field_index];
        match time_field.data_type {
            crate::types::DataType::UInt64 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const u64
                )
            ),
            crate::types::DataType::Timestamp => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const u64
                )
            ),
            crate::types::DataType::UInt8 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const u8
                ) as u64
            ),
            crate::types::DataType::UInt16 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const u16
                ) as u64
            ),
            crate::types::DataType::UInt32 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const u32
                ) as u64
            ),
            crate::types::DataType::Int8 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const i8
                ) as u64
            ),
            crate::types::DataType::Int16 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const i16
                ) as u64
            ),
            crate::types::DataType::Int32 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const i32
                ) as u64
            ),
            crate::types::DataType::Int64 => Some(
                core::ptr::read_unaligned(
                    record_ptr.add(time_field.offset) as *const i64
                ) as u64
            ),
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
        end_time: u64
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
        dest: *mut u8
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
            for j in i+1..record_count {
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
        max_records: usize
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
            for j in i+1..match_count {
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
        window_size: u64
    ) -> Result<Vec<(u64, f64, f64, f64, f64, usize)>> {
        // 检查字段索引有效性
        if time_field_index >= self.def.fields.len() || value_field_index >= self.def.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }
        
        let time_field = &self.def.fields[time_field_index];
        let value_field = &self.def.fields[value_field_index];
        
        // 检查时间字段类型
        if time_field.data_type != crate::types::DataType::Timestamp && 
           time_field.data_type != crate::types::DataType::UInt64 {
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
                let entry = window_aggregates.entry(window_key).or_insert((0.0, numeric_value, numeric_value, 0.0, 0));
                entry.0 += numeric_value; // sum
                if numeric_value < entry.1 { entry.1 = numeric_value; } // min
                if numeric_value > entry.2 { entry.2 = numeric_value; } // max
                entry.3 = numeric_value; // last
                entry.4 += 1; // count
            }
        }
        
        // 将聚合结果转换为向量
        let mut result = Vec::with_capacity(window_aggregates.len());
        for (window_start, (sum, min, max, last, count)) in window_aggregates {
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
        _window_size: u64
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
