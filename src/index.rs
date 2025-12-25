use core::ptr::NonNull;
use crate::types::{TableDef, Result, RemDbError};
use crate::platform::{memcpy, memset};

/// 索引统计信息
pub struct IndexStats {
    /// 索引使用次数
    pub access_count: usize,
    /// 索引命中次数
    pub hit_count: usize,
    /// 索引大小（字节）
    pub size: usize,
    /// 索引项数量
    pub item_count: usize,
}

/// 主键哈希索引项
#[repr(C)]
pub struct PrimaryIndexItem {
    /// 下一个项的指针
    pub next: Option<NonNull<PrimaryIndexItem>>,
    /// 记录ID
    pub record_id: u16,
    /// 键大小
    pub key_size: u8,
    /// 键数据
    pub key_data: [u8; 64], // 最大键大小64字节
}

/// 辅助索引项
#[derive(Copy, Clone)]
#[repr(C)]
pub struct SecondaryIndexItem {
    /// 键大小
    pub key_size: u8,
    /// 记录ID
    pub record_id: u16,
    /// 键数据
    pub key_data: [u8; 64], // 最大键大小64字节
}

/// 主键哈希索引
pub struct PrimaryIndex {
    /// 表定义
    def: &'static TableDef,
    /// 哈希表数组
    hash_table: NonNull<Option<NonNull<PrimaryIndexItem>>>,
    /// 哈希表大小
    hash_table_size: usize,
    /// 索引项数组
    items: NonNull<PrimaryIndexItem>,
    /// 可用索引项指针
    free_items: Option<NonNull<PrimaryIndexItem>>,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋锁
    lock: u32,
}

impl PrimaryIndex {
    /// 创建新的主键索引
    pub unsafe fn new(
        def: &'static TableDef,
        hash_table_start: *mut Option<NonNull<PrimaryIndexItem>>,
        items_start: *mut PrimaryIndexItem,
        hash_table_size: usize,
        max_items: usize
    ) -> Self {
        // 初始化哈希表
        let hash_table = NonNull::new_unchecked(hash_table_start);
        for i in 0..hash_table_size {
            let slot_ptr = hash_table.as_ptr().add(i);
            *slot_ptr = None;
        }
        
        // 初始化索引项链表
        let items = NonNull::new_unchecked(items_start);
        let mut free_items = None;
        for i in (0..max_items).rev() {
            let item_ptr = items.as_ptr().add(i);
            (*item_ptr).next = free_items;
            (*item_ptr).record_id = 0;
            (*item_ptr).key_size = 0;
            memset((*item_ptr).key_data.as_mut_ptr(), 0, 64);
            free_items = Some(NonNull::new_unchecked(item_ptr));
        }
        
        PrimaryIndex {
            def,
            hash_table,
            hash_table_size,
            items,
            free_items,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: hash_table_size * core::mem::size_of::<Option<NonNull<PrimaryIndexItem>>>() + 
                      max_items * core::mem::size_of::<PrimaryIndexItem>(),
                item_count: 0,
            },
            lock: 0,
        }
    }
    
    /// 计算主键索引所需的内存大小
    pub const fn calculate_memory_size(
        _def: &'static TableDef,
        hash_table_size: usize,
        max_items: usize
    ) -> usize {
        let hash_table_size_bytes = hash_table_size * core::mem::size_of::<Option<NonNull<PrimaryIndexItem>>>();
        let items_size_bytes = max_items * core::mem::size_of::<PrimaryIndexItem>();
        
        hash_table_size_bytes + items_size_bytes
    }
    
    /// 计算哈希值
    fn hash_key(&self, key: *const u8, key_size: usize) -> usize {
        // 优化的哈希算法：使用MurmurHash3的简化版本，更适合小数据
        let mut hash = 0u64;
        let seed = 0x5bd1e995u64;
        
        for i in 0..key_size {
            let byte = unsafe { *key.add(i) };
            hash ^= byte as u64;
            hash = hash.wrapping_mul(seed);
            hash ^= hash >> 47;
        }
        
        (hash as usize) % self.hash_table_size
    }
    
    /// 插入索引项
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 检查键大小
        if key_size > 64 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 查找空闲索引项
        let mut item = match self.free_items {
            Some(item_ptr) => {
                // 从空闲列表获取项
                let next = (*item_ptr.as_ptr()).next;
                self.free_items = next;
                item_ptr
            },
            None => {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::OutOfMemory);
            }
        };
        
        // 设置索引项
        let item_mut = item.as_mut();
        item_mut.record_id = record_id;
        item_mut.key_size = key_size as u8;
        memcpy(item_mut.key_data.as_mut_ptr(), key, key_size);
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);
        
        // 插入到哈希表槽位的头部
        item_mut.next = *slot_ptr;
        *slot_ptr = Some(item);
        
        // 更新统计信息
        self.stats.item_count += 1;
        
        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }
    
    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);
        
        // 遍历链表查找
        let mut current = *slot_ptr;
        while let Some(item) = current {
            let item_ref = item.as_ref();
            
            // 比较键
            if item_ref.key_size == key_size as u8 {
                let mut match_found = true;
                for i in 0..key_size {
                    if item_ref.key_data[i] != *key.add(i) {
                        match_found = false;
                        break;
                    }
                }
                
                if match_found {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(item_ref.record_id);
                }
            }
            
            current = item_ref.next;
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 删除索引项
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);
        
        // 遍历链表查找并删除
        let mut current = *slot_ptr;
        let mut prev: Option<NonNull<PrimaryIndexItem>> = None;
        
        while let Some(mut item) = current {
            let item_ref = item.as_ref();
            
            // 比较键
            if item_ref.key_size == key_size as u8 {
                let mut match_found = true;
                for i in 0..key_size {
                    if item_ref.key_data[i] != *key.add(i) {
                        match_found = false;
                        break;
                    }
                }
                
                if match_found {
                    // 从链表中移除
                    if let Some(mut prev_item) = prev {
                        prev_item.as_mut().next = item_ref.next;
                    } else {
                        *slot_ptr = item_ref.next;
                    }
                    
                    // 归还到空闲列表
                    let item_mut = item.as_mut();
                    item_mut.next = self.free_items;
                    self.free_items = Some(item);
                        
                        // 更新统计信息
                        self.stats.item_count -= 1;
                        
                        crate::platform::spin_unlock(&mut self.lock);
                        return Ok(());
                    }
                }
                
                prev = Some(item);
                current = item_ref.next;
        }
        
        crate::platform::spin_unlock(&mut self.lock);
        Err(RemDbError::RecordNotFound)
    }
    
    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }
    
    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        self.stats.access_count = 0;
        self.stats.hit_count = 0;
    }
}

/// 辅助有序索引
pub struct SecondaryIndex {
    /// 表定义
    def: &'static TableDef,
    /// 索引项数组
    items: NonNull<SecondaryIndexItem>,
    /// 当前项数量
    item_count: usize,
    /// 最大项数量
    max_items: usize,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋锁
    lock: u32,
}

impl SecondaryIndex {
    /// 创建新的辅助索引
    pub unsafe fn new(
        def: &'static TableDef,
        items_start: *mut SecondaryIndexItem,
        max_items: usize
    ) -> Self {
        let items = NonNull::new_unchecked(items_start);
        
        SecondaryIndex {
            def,
            items,
            item_count: 0,
            max_items,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: max_items * core::mem::size_of::<SecondaryIndexItem>(),
                item_count: 0,
            },
            lock: 0,
        }
    }
    
    /// 计算辅助索引所需的内存大小
    pub const fn calculate_memory_size(max_items: usize) -> usize {
        max_items * core::mem::size_of::<SecondaryIndexItem>()
    }
    
    /// 比较两个索引项
    fn compare_items(
        &self,
        item1: &SecondaryIndexItem,
        item2: &SecondaryIndexItem
    ) -> core::cmp::Ordering {
        // 比较键大小
        if item1.key_size != item2.key_size {
            return item1.key_size.cmp(&item2.key_size);
        }
        
        // 比较键数据
        let key_size = item1.key_size as usize;
        for i in 0..key_size {
            if item1.key_data[i] != item2.key_data[i] {
                return item1.key_data[i].cmp(&item2.key_data[i]);
            }
        }
        
        // 键相等，比较记录ID
        item1.record_id.cmp(&item2.record_id)
    }
    
    /// 二分查找索引项
    fn binary_search(
        &self,
        key: *const u8,
        key_size: usize
    ) -> Result<usize> {
        if self.item_count == 0 {
            return Err(RemDbError::RecordNotFound);
        }
        
        let mut low = 0;
        let mut high = self.item_count - 1;
        
        while low <= high {
            let mid = (low + high) / 2;
            let mid_item = unsafe { &*self.items.as_ptr().add(mid) };
            
            // 比较键
            let cmp = if mid_item.key_size != key_size as u8 {
                mid_item.key_size.cmp(&(key_size as u8))
            } else {
                let mut equal = true;
                for i in 0..key_size {
                    if mid_item.key_data[i] != unsafe { *key.add(i) } {
                        equal = false;
                        break;
                    }
                }
                if equal {
                    core::cmp::Ordering::Equal
                } else {
                    // 逐个字节比较
                    let mut ordering = core::cmp::Ordering::Equal;
                    for i in 0..key_size {
                        let b1 = mid_item.key_data[i];
                        let b2 = unsafe { *key.add(i) };
                        if b1 != b2 {
                            ordering = b1.cmp(&b2);
                            break;
                        }
                    }
                    ordering
                }
            };
            
            match cmp {
                core::cmp::Ordering::Equal => return Ok(mid),
                core::cmp::Ordering::Less => low = mid + 1,
                core::cmp::Ordering::Greater => {
                    if mid == 0 {
                        return Err(RemDbError::RecordNotFound);
                    }
                    high = mid - 1;
                }
            }
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 插入索引项
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()>
    {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 检查是否已满
        if self.item_count >= self.max_items {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::OutOfMemory);
        }
        
        // 检查键大小
        if key_size > 64 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建新索引项
        let new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 64],
        };
        memcpy(new_item.key_data.as_ptr() as *mut u8, key, key_size);
        
        // 使用二分查找找到插入位置，将O(n)优化为O(log n)
        let mut insert_pos = self.item_count;
        if self.item_count > 0 {
            let mut low = 0;
            let mut high = self.item_count - 1;
            
            while low <= high {
                let mid = (low + high) / 2;
                let item = &*self.items.as_ptr().add(mid);
                
                match self.compare_items(&new_item, item) {
                    core::cmp::Ordering::Less => {
                        insert_pos = mid;
                        high = mid - 1;
                    }
                    core::cmp::Ordering::Greater => {
                        low = mid + 1;
                    }
                    core::cmp::Ordering::Equal => {
                        // 插入到相等元素后面
                        insert_pos = mid + 1;
                        low = mid + 1;
                    }
                }
            }
        }
        
        // 移动现有项为新项腾出空间
        if insert_pos < self.item_count {
            // 从后往前拷贝，避免覆盖数据
            for i in (insert_pos..self.item_count).rev() {
                let src = self.items.as_ptr().add(i);
                let dest = self.items.as_ptr().add(i + 1);
                *dest = *src;
            }
        }
        
        // 插入新项
        let insert_ptr = self.items.as_ptr().add(insert_pos);
        *insert_ptr = new_item;
        
        // 更新统计信息
        self.item_count += 1;
        self.stats.item_count = self.item_count;
        
        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }
    
    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        match self.binary_search(key, key_size) {
            Ok(index) => {
                // 更新命中统计
                self.stats.hit_count += 1;
                Ok((*self.items.as_ptr().add(index)).record_id)
            },
            Err(e) => Err(e),
        }
    }
    
    /// 删除索引项
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()>
    {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        let result = match self.binary_search(key, key_size) {
            Ok(index) => {
                // 移动后续项覆盖被删除项
                if index < self.item_count - 1 {
                    let dest_ptr = self.items.as_ptr().add(index);
                    let src_ptr = self.items.as_ptr().add(index + 1);
                    let move_size = (self.item_count - index - 1) * core::mem::size_of::<SecondaryIndexItem>();
                    memcpy(dest_ptr as *mut u8, src_ptr as *const u8, move_size);
                }
                
                // 清空最后一项
                let last_ptr = self.items.as_ptr().add(self.item_count - 1);
                memset(last_ptr as *mut u8, 0, core::mem::size_of::<SecondaryIndexItem>());
                
                // 更新统计信息
                self.item_count -= 1;
                self.stats.item_count = self.item_count;
                
                Ok(())
            },
            Err(e) => Err(e),
        };
        
        crate::platform::spin_unlock(&mut self.lock);
        result
    }
    
    /// 范围查询（返回第一个匹配项）
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.item_count - 1;
        
        // 创建临时索引项用于比较
        let start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_ptr() as *mut u8, start_key, start_key_size);
        
        // 二分查找起始位置
        while low <= high {
            let mid = (low + high) / 2;
            let item = &*self.items.as_ptr().add(mid);
            
            if self.compare_items(item, &start_item) == core::cmp::Ordering::Less {
                start_pos = mid + 1;
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
        
        // 从起始位置开始遍历，直到找到匹配项或超出范围
        for i in start_pos..self.item_count {
            let item = &*self.items.as_ptr().add(i);
            
            // 检查是否小于等于end_key
            let mut le_end = false;
            let key_size = item.key_size as usize;
            
            if key_size > end_key_size {
                // 键大小大于end_key，超出范围
                break;
            }
            
            // 比较键数据
            let min_size = core::cmp::min(key_size, end_key_size);
            let mut all_equal = true;
            
            for j in 0..min_size {
                if item.key_data[j] < *end_key.add(j) {
                    le_end = true;
                    break;
                } else if item.key_data[j] > *end_key.add(j) {
                    // 超出范围
                    le_end = false;
                    all_equal = false;
                    break;
                }
            }
            
            if all_equal && key_size == end_key_size {
                le_end = true;
            }
            
            if le_end {
                // 更新命中统计
                self.stats.hit_count += 1;
                return Ok(item.record_id);
            } else {
                // 已超出范围，结束遍历
                break;
            }
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 范围查询（返回所有匹配项）
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查输出缓冲区是否为null
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 如果没有记录，直接返回
        if self.item_count == 0 {
            return Ok(0);
        }
        
        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.item_count - 1;
        
        // 创建临时索引项用于比较
        let start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_ptr() as *mut u8, start_key, start_key_size);
        
        // 二分查找起始位置
        while low <= high {
            let mid = (low + high) / 2;
            let item = &*self.items.as_ptr().add(mid);
            
            if self.compare_items(item, &start_item) == core::cmp::Ordering::Less {
                start_pos = mid + 1;
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
        
        // 从起始位置开始遍历，收集所有匹配项
        let mut match_count = 0;
        for i in start_pos..self.item_count {
            if match_count >= max_records {
                break;
            }
            
            let item = &*self.items.as_ptr().add(i);
            
            // 检查是否小于等于end_key
            let mut le_end = false;
            let key_size = item.key_size as usize;
            
            if key_size > end_key_size {
                // 键大小大于end_key，超出范围
                break;
            }
            
            // 比较键数据
            let min_size = core::cmp::min(key_size, end_key_size);
            let mut all_equal = true;
            
            for j in 0..min_size {
                if item.key_data[j] < *end_key.add(j) {
                    le_end = true;
                    break;
                } else if item.key_data[j] > *end_key.add(j) {
                    // 超出范围
                    le_end = false;
                    all_equal = false;
                    break;
                }
            }
            
            if all_equal && key_size == end_key_size {
                le_end = true;
            }
            
            if le_end {
                // 保存匹配项的record_id到输出缓冲区
                *out_record_ids.add(match_count) = item.record_id;
                match_count += 1;
            } else {
                // 已超出范围，结束遍历
                break;
            }
        }
        
        // 更新命中统计
        if match_count > 0 {
            self.stats.hit_count += match_count;
        }
        
        Ok(match_count)
    }
    
    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }
    
    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        self.stats.access_count = 0;
        self.stats.hit_count = 0;
    }
    
    /// 获取当前项数量
    pub fn item_count(&self) -> usize {
        self.item_count
    }
    
    /// 获取最大项数量
    pub fn max_items(&self) -> usize {
        self.max_items
    }
}
