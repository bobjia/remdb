#!/usr/bin/env python3
import io

NEW_TAIL = '''
/// 活跃事务快照
struct ActiveSnapshot {
    /// 事务ID
    tx_id: u32,
    /// 快照版本号
    snapshot_version: u32,
}

/// 事务管理器
pub struct TransactionManager {
    /// 当前活动事务
    current_tx: Option<NonNull<Transaction>>,
    /// 事务ID计数器
    pub tx_id_counter: u32,
    /// 全局快照版本号
    pub snapshot_version: u32,
    /// 活跃事务快照列表
    active_snapshots: alloc::vec::Vec<ActiveSnapshot>,
    /// 自旋锁
    lock: parking_lot::Mutex<()>,
    /// 日志管理器
    log_manager: Option<LogManager>,
    /// 低功耗模式标志
    low_power_mode: bool,
}

// Transaction 和 TransactionManager 包含 NonNull 指针，需要手动实现 Send/Sync
unsafe impl Send for Transaction {}
unsafe impl Sync for Transaction {}
unsafe impl Send for TransactionManager {}
unsafe impl Sync for TransactionManager {}

impl TransactionManager {
    /// 创建新的事务管理器
    pub const fn new() -> Self {
        Self {
            current_tx: None,
            tx_id_counter: 1,
            snapshot_version: 0,
            active_snapshots: alloc::vec::Vec::new(),
            lock: parking_lot::Mutex::new(()),
            log_manager: None,
            low_power_mode: false,
        }
    }

    /// 设置日志管理器
    pub fn set_log_manager(&mut self, log_manager: LogManager) {
        self.log_manager = Some(log_manager);
    }

    /// 清除日志管理器
    pub fn clear_log_manager(&mut self) {
        self.log_manager = None;
    }

    /// 获取日志管理器（可变）
    pub fn get_log_manager_mut(&mut self) -> Option<&mut LogManager> {
        self.log_manager.as_mut()
    }

    /// 获取日志管理器（只读）
    pub fn get_log_manager(&self) -> Option<&LogManager> {
        self.log_manager.as_ref()
    }

    /// 刷新所有日志
    pub fn flush_logs(&mut self) -> Result<()> {
        if let Some(log_manager) = &mut self.log_manager {
            log_manager.flush_buffer()
        } else {
            Ok(())
        }
    }

    /// 检查是否有活动事务
    pub fn has_active_tx(&self) -> bool {
        self.current_tx.is_some()
    }

    /// 获取当前事务ID计数器
    pub fn tx_id_counter(&self) -> u32 {
        self.tx_id_counter
    }

    /// 开始事务
    pub fn begin(
        &mut self,
        tx_type: TransactionType,
        isolation_level: IsolationLevel,
        tx_buffer: *mut Transaction,
        log_buffer: *mut LogItem,
        max_log_items: usize,
    ) -> Result<NonNull<Transaction>> {
        let _lock = self.lock.lock();

        // 检查是否已经有活跃事务（不支持嵌套事务）
        if self.current_tx.is_some() {
            return Err(RemDbError::TransactionError);
        }

        // 更新事务ID计数器
        let tx_id = self.tx_id_counter;
        self.tx_id_counter += 1;

        // 生成快照版本号
        let snapshot_version = self.snapshot_version;

        // 检查外部缓冲区是否有效
        if !tx_buffer.is_null() && !log_buffer.is_null() && max_log_items > 0 {
            // 测试环境：使用外部提供的缓冲区初始化事务对象
            (*tx_buffer).id = tx_id;
            (*tx_buffer).tx_type = tx_type;
            (*tx_buffer).status = TransactionStatus::Active;
            (*tx_buffer).isolation_level = isolation_level;
            (*tx_buffer).start_time = crate::platform::get_timestamp_us();
            (*tx_buffer).log_items = NonNull::new_unchecked(log_buffer);
            (*tx_buffer).max_log_items = max_log_items;
            (*tx_buffer).log_item_count = 0;
            (*tx_buffer).depth = 1;
            (*tx_buffer).lock = parking_lot::Mutex::new(());

            // 保存当前事务引用
            self.current_tx = Some(NonNull::new_unchecked(tx_buffer));

            // 添加事务快照到活跃快照列表
            self.active_snapshots.push(ActiveSnapshot {
                tx_id,
                snapshot_version,
            });

            Ok(NonNull::new_unchecked(tx_buffer))
        } else {
            // JDBC服务器环境：只跟踪事务状态，不使用外部缓冲区
            self.current_tx = Some(NonNull::dangling());

            // 添加事务快照到活跃快照列表
            self.active_snapshots.push(ActiveSnapshot {
                tx_id,
                snapshot_version,
            });

            Ok(NonNull::dangling())
        }
    }

    /// 提交事务
    pub fn commit(&mut self) -> Result<()> {
        // 增加已提交事务计数
        crate::get_global_db().map(|db| db.metrics.inc_committed_transactions());
        let _lock = self.lock.lock();

        // 检查是否有活跃事务
        let tx_ptr = match self.current_tx.take() {
            Some(tx) => tx,
            None => return Err(RemDbError::TransactionError),
        };

        // 检查是否是悬垂指针（用于JDBC服务器）
        let is_dangling = tx_ptr.as_ptr() == NonNull::dangling().as_ptr();

        let tx_id = if !is_dangling {
            // 测试环境：更新事务状态
            let tx = &mut *tx_ptr.as_ptr();

            // 记录提交日志
            if let Some(log_manager) = &mut self.log_manager {
                let log_item = LogItem {
                    op_type: LogOperation::Commit,
                    table_id: 0,
                    record_id: 0,
                    data_size: 0,
                    old_data: [0; 512],
                    new_data: [0; 512],
                    tx_id: tx.id,
                    timestamp: crate::platform::get_timestamp_us(),
                    checksum: 0,
                };

                let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
                core::ptr::write_unaligned(log_bytes.as_mut_ptr() as *mut LogItem, log_item);
                let mut check_bytes = log_bytes.clone();
                let checksum_ptr = check_bytes.as_mut_ptr().add(core::mem::size_of::<LogItem>() - 4) as *mut u32;
                *checksum_ptr = 0;
                let calculated_checksum = Transaction::calculate_checksum(&check_bytes);

                let mut final_log_item = log_item;
                final_log_item.checksum = calculated_checksum;

                log_manager.write_log_item(&final_log_item)?;
            }

            tx.status = TransactionStatus::Committed;
            tx.id
        } else {
            self.tx_id_counter - 1
        };

        // 移除事务快照从活跃快照列表
        self.active_snapshots.retain(|snapshot| snapshot.tx_id != tx_id);
        // 增加全局快照版本号
        self.snapshot_version += 1;

        Ok(())
    }

    /// 回滚事务
    pub fn rollback(&mut self, db: &mut crate::RemDb) -> Result<()> {
        crate::get_global_db().map(|db| db.metrics.inc_rolled_back_transactions());
        let _lock = self.lock.lock();

        let tx_ptr = match self.current_tx.take() {
            Some(tx) => tx,
            None => return Err(RemDbError::TransactionError),
        };

        let is_dangling = tx_ptr.as_ptr() == NonNull::dangling().as_ptr();

        if !is_dangling {
            let tx = &mut *tx_ptr.as_ptr();

            for i in (0..tx.log_item_count).rev() {
                let log_ptr = tx.log_items.as_ptr().add(i);
                let log_item = *log_ptr;

                match log_item.op_type {
                    LogOperation::Insert => {
                        let table = match &mut db.tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
                        };
                        let record_id = log_item.record_id as usize;
                        if table.status_array[record_id].status == crate::types::RecordStatus::Used {
                            table.status_array[record_id].status = crate::types::RecordStatus::Free;
                            table.status_array[record_id].version += 1;
                            let record_slice = table.get_record_slice_mut(record_id);
                            let data_size = log_item.data_size as usize;
                            crate::platform::memset(&mut record_slice[..data_size], 0);
                            drop(record_slice);
                            table.free_slots.push(log_item.record_id as usize);
                            table.record_count -= 1;
                        }
                    },
                    LogOperation::Delete => {
                        let table = match &mut db.tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
                        };
                        let record_id = log_item.record_id as usize;
                        if table.status_array[record_id].status == crate::types::RecordStatus::Free {
                            table.status_array[record_id].status = crate::types::RecordStatus::Used;
                            table.status_array[record_id].version += 1;
                            let record_slice = table.get_record_slice_mut(record_id);
                            let data_size = log_item.data_size as usize;
                            crate::platform::memcpy(&mut record_slice[..data_size], &log_item.old_data[..data_size]);
                            drop(record_slice);
                            table.record_count += 1;
                        }
                    },
                    LogOperation::Update => {
                        let table = match &mut db.tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
                        };
                        let record_id = log_item.record_id as usize;
                        if table.status_array[record_id].status == crate::types::RecordStatus::Used {
                            let record_slice = table.get_record_slice_mut(record_id);
                            let data_size = log_item.data_size as usize;
                            crate::platform::memcpy(&mut record_slice[..data_size], &log_item.old_data[..data_size]);
                            drop(record_slice);
                            table.status_array[record_id].version += 1;
                        }
                    },
                    LogOperation::TimeSeriesInsert => {
                        let ts_table = match &mut db.time_series_tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
                        };
                        let mut record = crate::time_series::TimeSeriesRecord {
                            timestamp: 0,
                            value: 0.0,
                            tag_count: 0,
                            tags: [0; 8],
                        };
                        let size = core::mem::size_of::<crate::time_series::TimeSeriesRecord>();
                        let record_bytes = unsafe {
                            core::slice::from_raw_parts_mut(&mut record as *mut _ as *mut u8, size)
                        };
                        crate::platform::memcpy(record_bytes, &log_item.new_data[..size]);

                        let partitions_guard = ts_table.partitions.lock().unwrap();
                        if let Some(partition) = partitions_guard.get_partition(record.timestamp) {
                            let mut partition_guard = partition.lock().unwrap();
                            if let Some(index) = partition_guard.records.iter().position(|r| r.timestamp == record.timestamp) {
                                partition_guard.records.remove(index);
                                partition_guard.stats.record_count = partition_guard.records.len();
                                ts_table.index.remove(record.timestamp);
                            }
                        }
                    },
                    _ => continue,
                }
            }

            // 记录回滚日志
            if let Some(log_manager) = &mut self.log_manager {
                let log_item = LogItem {
                    op_type: LogOperation::Abort,
                    table_id: 0,
                    record_id: 0,
                    data_size: 0,
                    old_data: [0; 512],
                    new_data: [0; 512],
                    tx_id: tx.id,
                    timestamp: crate::platform::get_timestamp_us(),
                    checksum: 0,
                };

                let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
                core::ptr::write_unaligned(log_bytes.as_mut_ptr() as *mut LogItem, log_item);
                let mut check_bytes = log_bytes.clone();
                let checksum_ptr = check_bytes.as_mut_ptr().add(core::mem::size_of::<LogItem>() - 4) as *mut u32;
                *checksum_ptr = 0;
                let calculated_checksum = Transaction::calculate_checksum(&check_bytes);

                let mut final_log_item = log_item;
                final_log_item.checksum = calculated_checksum;

                log_manager.write_log_item(&final_log_item)?;
            }

            tx.status = TransactionStatus::RolledBack;
        }

        let tx_id = if !is_dangling {
            let tx = &mut *tx_ptr.as_ptr();
            tx.id
        } else {
            self.tx_id_counter - 1
        };

        // 移除事务快照从活跃快照列表
        self.active_snapshots.retain(|snapshot| snapshot.tx_id != tx_id);

        Ok(())
    }

    /// 获取当前事务
    pub fn get_current_tx(&self) -> Option<NonNull<Transaction>> {
        self.current_tx
    }

    /// 可见性判断：检查记录版本是否对当前事务可见
    pub fn is_visible(&self, create_tx_id: u32, delete_tx_id: u32, tx_id: u32) -> bool {
        create_tx_id <= tx_id && (delete_tx_id == 0 || delete_tx_id > tx_id)
    }

    /// 获取活跃事务中最小的事务ID
    pub fn get_min_active_tx_id(&self) -> u32 {
        if self.active_snapshots.is_empty() {
            self.tx_id_counter
        } else {
            self.active_snapshots.iter().map(|s| s.tx_id).min().unwrap_or(self.tx_id_counter)
        }
    }

    /// 垃圾回收检查：判断记录版本是否可以被回收
    pub fn can_recycle(&self, create_tx_id: u32, delete_tx_id: u32) -> bool {
        let min_active_tx_id = self.get_min_active_tx_id();
        create_tx_id < min_active_tx_id && (delete_tx_id == 0 || delete_tx_id < min_active_tx_id)
    }

    /// 检测写入冲突：检查记录是否被其他事务修改
    pub fn detect_write_conflict(&self, create_tx_id: u32, current_tx_id: u32) -> bool {
        create_tx_id > current_tx_id
    }

    /// 设置低功耗模式
    pub fn set_low_power_mode(&mut self, enabled: bool) {
        self.low_power_mode = enabled;
    }

    /// 获取低功耗模式状态
    pub fn is_low_power_mode(&self) -> bool {
        self.low_power_mode
    }

    /// 重置事务管理器
    pub fn reset(&mut self) {
        self.current_tx = None;
        self.active_snapshots.clear();
        self.tx_id_counter = 1;
        self.snapshot_version = 0;
    }
}

impl Transaction {
    /// 计算数据校验和
    pub fn calculate_checksum(data: &[u8]) -> u32 {
        let mut checksum = 0u32;
        for &byte in data {
            checksum ^= byte as u32;
        }
        checksum
    }

    /// 添加日志项
    pub unsafe fn add_log_item(
        &mut self,
        op_type: LogOperation,
        table_id: u8,
        record_id: u16,
        old_data: *const u8,
        new_data: *const u8,
        data_size: usize,
    ) -> Result<()> {
        let _lock = self.lock.lock();

        // 检查事务状态
        if self.status != TransactionStatus::Active {
            return Err(RemDbError::TransactionError);
        }

        // 检查数据大小
        if data_size > 512 {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 获取日志项指针
        let log_ptr = self.log_items.as_ptr().add(self.log_item_count);

        // 设置日志项基本信息
        (*log_ptr).op_type = op_type;
        (*log_ptr).table_id = table_id;
        (*log_ptr).record_id = record_id;
        (*log_ptr).data_size = data_size as u16;
        (*log_ptr).tx_id = self.id;
        (*log_ptr).timestamp = crate::platform::get_timestamp_us();

        // 拷贝旧数据
        if old_data.is_null() {
            memset(&mut (&mut (*log_ptr).old_data)[..data_size], 0);
        } else {
            let old_slice = unsafe { core::slice::from_raw_parts(old_data, data_size) };
            memcpy(&mut (&mut (*log_ptr).old_data)[..data_size], old_slice);
        }

        // 拷贝新数据
        if new_data.is_null() {
            memset(&mut (&mut (*log_ptr).new_data)[..data_size], 0);
        } else {
            let new_slice = unsafe { core::slice::from_raw_parts(new_data, data_size) };
            memcpy(&mut (&mut (*log_ptr).new_data)[..data_size], new_slice);
        }

        // 计算校验和
        let mut checksum_data = [0u8; 1024];
        let mut offset = 0;
        checksum_data[offset] = (*log_ptr).op_type as u8;
        offset += 1;
        checksum_data[offset] = (*log_ptr).table_id;
        offset += 1;
        checksum_data[offset..offset + 2].copy_from_slice(&(*log_ptr).record_id.to_le_bytes());
        offset += 2;
        checksum_data[offset..offset + 2].copy_from_slice(&(*log_ptr).data_size.to_le_bytes());
        offset += 2;
        checksum_data[offset..offset + data_size].copy_from_slice(&(&(*log_ptr).old_data)[0..data_size]);
        offset += data_size;
        checksum_data[offset..offset + data_size].copy_from_slice(&(&(*log_ptr).new_data)[0..data_size]);
        offset += data_size;
        (*log_ptr).checksum = Self::calculate_checksum(&checksum_data[0..offset]);

        // 更新日志项计数
        self.log_item_count += 1;

        Ok(())
    }

    /// 开始事务日志项
    pub unsafe fn begin_log_item(
        &mut self,
        tx_id: u32,
        op_type: LogOperation,
        table_id: u8,
        record_id: u16,
        data_size: u16,
        old_data: Option<&[u8]>,
        new_data: Option<&[u8]>,
    ) -> Option<NonNull<LogItem>> {
        if self.log_item_count >= self.max_log_items {
            return None;
        }

        let log_item_ptr = self.log_items.as_ptr().add(self.log_item_count);
        let log_item = log_item_ptr.as_mut().unwrap();

        log_item.op_type = op_type;
        log_item.table_id = table_id;
        log_item.record_id = record_id;
        log_item.data_size = data_size;
        log_item.old_data = [0; 512];
        log_item.new_data = [0; 512];
        log_item.tx_id = tx_id;
        log_item.timestamp = crate::platform::get_timestamp_us();
        log_item.checksum = 0;

        if let Some(data) = old_data {
            let copy_len = core::cmp::min(data.len(), 512);
            log_item.old_data[..copy_len].copy_from_slice(&data[..copy_len]);
        }
        if let Some(data) = new_data {
            let copy_len = core::cmp::min(data.len(), 512);
            log_item.new_data[..copy_len].copy_from_slice(&data[..copy_len]);
        }

        let calculated_checksum = Transaction::calculate_log_item_checksum(log_item);
        log_item.checksum = calculated_checksum;

        self.log_item_count += 1;
        Some(NonNull::new_unchecked(log_item_ptr))
    }

    /// 检查事务是否活跃
    pub fn is_active(&self) -> bool {
        self.status == TransactionStatus::Active
    }

    /// 检查事务是否为只读
    pub fn is_read_only(&self) -> bool {
        self.tx_type == TransactionType::ReadOnly
    }
}

/// 事务管理器全局实例
static mut TX_MANAGER: TransactionManager = TransactionManager::new();

/// 获取全局事务管理器
pub fn get_tx_manager() -> &'static mut TransactionManager {
    unsafe {
        &mut TX_MANAGER
    }
}

/// 初始化事务管理器
pub fn init_tx_manager() {
    unsafe {
        TX_MANAGER.reset();
    }
}

/// 刷新所有日志
pub fn flush_all_logs() -> Result<()> {
    unsafe {
        TX_MANAGER.flush_logs()
    }
}

/// 设置全局日志管理器
pub fn set_log_manager(log_manager: LogManager) {
    unsafe {
        TX_MANAGER.set_log_manager(log_manager);
    }
}

/// 获取全局日志管理器
pub fn get_log_manager() -> Option<&'static mut LogManager> {
    unsafe {
        TX_MANAGER.get_log_manager_mut()
    }
}

/// 重置全局日志管理器
pub fn reset_log_manager() {
    unsafe {
        TX_MANAGER.clear_log_manager();
    }
}

/// 设置事务管理器低功耗模式
pub fn set_low_power_mode(enabled: bool) {
    unsafe {
        TX_MANAGER.set_low_power_mode(enabled);
    }
}

/// 获取事务管理器低功耗模式状态
pub fn is_low_power_mode() -> bool {
    unsafe {
        TX_MANAGER.is_low_power_mode()
    }
}

/// 检查是否有活跃事务
pub fn has_active_tx() -> bool {
    unsafe {
        TX_MANAGER.has_active_tx()
    }
}

/// 获取当前事务
pub fn get_current_tx() -> Option<NonNull<Transaction>> {
    unsafe {
        TX_MANAGER.get_current_tx()
    }
}

/// 开始事务
pub fn begin(
    tx_type: TransactionType,
    isolation_level: IsolationLevel,
    tx_buffer: *mut Transaction,
    log_buffer: *mut LogItem,
    max_log_items: usize,
) -> Result<NonNull<Transaction>> {
    unsafe {
        TX_MANAGER.begin(tx_type, isolation_level, tx_buffer, log_buffer, max_log_items)
    }
}

/// 提交事务
pub fn commit() -> Result<()> {
    unsafe {
        TX_MANAGER.commit()
    }
}

/// 回滚事务
pub fn rollback(db: &mut crate::RemDb) -> Result<()> {
    unsafe {
        TX_MANAGER.rollback(db)
    }
}

/// 检查记录是否对当前事务可见（MVCC实现）
pub fn is_visible(create_tx_id: u32, delete_tx_id: u32, current_tx_id: u32) -> bool {
    unsafe {
        TX_MANAGER.is_visible(create_tx_id, delete_tx_id, current_tx_id)
    }
}

/// 获取当前事务ID计数器
pub fn get_tx_id_counter() -> u32 {
    unsafe {
        TX_MANAGER.tx_id_counter()
    }
}

/// 检查记录是否可见（MVCC实现）
pub fn is_record_visible(create_tx_id: u32, delete_tx_id: u32, tx_id: u32) -> bool {
    unsafe {
        TX_MANAGER.is_visible(create_tx_id, delete_tx_id, tx_id)
    }
}

/// 使用日志管理器执行操作
pub fn with_log_manager<F, R>(f: F) -> R
where
    F: FnOnce(Option<&mut LogManager>) -> R,
{
    unsafe {
        f(TX_MANAGER.get_log_manager_mut())
    }
}

/// 重置事务管理器
pub fn reset_tx_manager() {
    unsafe {
        TX_MANAGER.reset();
    }
}

/// 清除日志管理器
pub fn clear_log_manager_tx() {
    unsafe {
        TX_MANAGER.clear_log_manager();
    }
}
'''

src = '/workspace/src/transaction.rs'
with io.open(src, 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Keep lines 1..1855 (indices 0..1854), discard the corrupted remainder
new_content = ''.join(lines[:1855]) + '\n' + NEW_TAIL + '\n'

with io.open(src, 'w', encoding='utf-8', newline='\n') as f:
    f.write(new_content)

print('orig lines:', len(lines))
print('new lines:', new_content.count('\n'))