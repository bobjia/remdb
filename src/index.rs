use crate::types::{TableDef, Result, RemDbError, IndexType};

/// B-Tree阶数（每个节点的最大键数量）
const BTREE_ORDER: usize = 4; // 阶数为4的B-Tree

/// T-Tree阶数（每个节点的最大键数量）
const TTREE_ORDER: usize = 3; // 阶数为3的T-Tree

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

impl Default for SecondaryIndexItem {
    fn default() -> Self {
        SecondaryIndexItem {
            key_size: 0,
            record_id: 0,
            key_data: [0u8; 64],
        }
    }
}

/// B-Tree节点结构
pub struct BTreeNode {
    /// 节点类型（内部节点/叶子节点）
    pub is_leaf: bool,
    /// 当前键数量
    pub key_count: u8,
    /// 键数据（每个键64字节）
    pub keys: [SecondaryIndexItem; BTREE_ORDER],
    /// 子节点指针（仅内部节点使用）
    pub children: [Option<alloc::boxed::Box<BTreeNode>>; BTREE_ORDER + 1],
}

/// B-Tree索引结构
pub struct BTreeIndex {
    /// 表定义
    pub def: alloc::sync::Arc<TableDef>,
    /// 根节点
    pub root: Option<alloc::boxed::Box<BTreeNode>>,
    /// 索引统计信息
    pub stats: IndexStats,
    /// 自旋锁
    pub lock: parking_lot::Mutex<()>,
}

/// T-Tree节点结构
pub struct TTreeNode {
    /// 当前键数量
    pub key_count: u8,
    /// 键数据（每个键64字节）
    pub keys: [SecondaryIndexItem; TTREE_ORDER],
    /// 左子节点
    pub left: Option<alloc::boxed::Box<TTreeNode>>,
    /// 中子节点（用于T-Tree的三元分支）
    pub middle: Option<alloc::boxed::Box<TTreeNode>>,
    /// 右子节点
    pub right: Option<alloc::boxed::Box<TTreeNode>>,
}

/// T-Tree索引结构
pub struct TTreeIndex {
    /// 表定义
    pub def: alloc::sync::Arc<TableDef>,
    /// 根节点
    pub root: Option<alloc::boxed::Box<TTreeNode>>,
    /// 索引统计信息
    pub stats: IndexStats,
    /// 自旋锁
    pub lock: parking_lot::Mutex<()>,
}

/// 主键哈希索引
pub struct PrimaryIndex {
    /// 表定义
    def: alloc::sync::Arc<TableDef>,
    /// 哈希表（Vec<Vec<PrimaryIndexItem>> 每个桶是一个Vec）
    buckets: alloc::vec::Vec<alloc::vec::Vec<PrimaryIndexItem>>,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋锁
    lock: parking_lot::Mutex<()>,
}

impl PrimaryIndex {
    /// 创建新的主键索引
    pub fn new(
        def: alloc::sync::Arc<TableDef>,
        max_records: usize,
    ) -> Self {
        let hash_table_size = (max_records * 2).next_power_of_two();
        let buckets = (0..hash_table_size).map(|_| alloc::vec::Vec::new()).collect();
        
        PrimaryIndex {
            def,
            buckets,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: 0, // Vec管理自己的内存
                item_count: 0,
            },
            lock: parking_lot::Mutex::new(()),
        }
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
        
        (hash as usize) % self.buckets.len()
    }
    
    /// 插入索引项
    pub fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        // 检查键大小
        if key_size > 64 {
            // 锁会自动释放
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 设置索引项
        let mut item = PrimaryIndexItem {
            record_id,
            key_size: key_size as u8,
            key_data: [0u8; 64],
        };
        for i in 0..key_size {
            item.key_data[i] = unsafe { *key.add(i) };
        }
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        
        // 插入到哈希表槽位
        self.buckets[hash].push(item);
        
        // 更新统计信息
        self.stats.item_count += 1;
        
        // 锁会自动释放
        Ok(())
    }
    
    /// 根据键查找记录ID
    pub fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        
        // 遍历桶查找
        for item in &self.buckets[hash] {
            // 比较键
            if item.key_size == key_size as u8 {
                let mut match_found = true;
                for i in 0..key_size {
                    if item.key_data[i] != unsafe { *key.add(i) } {
                        match_found = false;
                        break;
                    }
                }
                
                if match_found {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(item.record_id);
                }
            }
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 删除索引项
    pub fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        let bucket = &mut self.buckets[hash];
        
        // 遍历桶查找并删除
        for i in (0..bucket.len()).rev() {
            let item = &bucket[i];
            
            // 比较键
            if item.key_size == key_size as u8 {
                let mut match_found = true;
                for j in 0..key_size {
                    if item.key_data[j] != unsafe { *key.add(j) } {
                        match_found = false;
                        break;
                    }
                }
                
                if match_found {
                    // 从桶中移除
                    bucket.remove(i);
                    
                    // 更新统计信息
                    self.stats.item_count -= 1;
                    
                    // 锁会自动释放
                    return Ok(());
                }
            }
        }
        
        // 锁会自动释放
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

/// 辅助索引枚举（用于封装不同类型的辅助索引）
pub enum AnySecondaryIndex {
    /// 有序数组索引
    SortedArray(SecondaryIndex),
    /// B-Tree索引
    BTree(BTreeIndex),
    /// T-Tree索引
    TTree(TTreeIndex),
}

impl AnySecondaryIndex {
    /// 创建新的辅助索引
    pub fn new(
        def: alloc::sync::Arc<TableDef>,
        max_items: usize,
    ) -> Result<Self> {
        match def.secondary_index_type {
            IndexType::SortedArray => {
                // 创建有序数组索引
                let index = SecondaryIndex::new(def, max_items);
                Ok(AnySecondaryIndex::SortedArray(index))
            },
            IndexType::BTree => {
                // 创建B-Tree索引
                let index = BTreeIndex::new(def);
                Ok(AnySecondaryIndex::BTree(index))
            },
            IndexType::TTree => {
                // 创建T-Tree索引
                let index = TTreeIndex::new(def);
                Ok(AnySecondaryIndex::TTree(index))
            },
            _ => {
                Err(RemDbError::UnsupportedOperation)
            }
        }
    }
    
    /// 插入索引项
    pub fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.insert(key, key_size, record_id),
            AnySecondaryIndex::BTree(index) => index.insert(key, key_size, record_id),
            AnySecondaryIndex::TTree(index) => index.insert(key, key_size, record_id),
        }
    }
    
    /// 根据键查找记录ID
    pub fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find(key, key_size),
            AnySecondaryIndex::BTree(index) => index.find(key, key_size),
            AnySecondaryIndex::TTree(index) => index.find(key, key_size),
        }
    }
    
    /// 范围查询（返回第一个匹配项）
    pub fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find_range(start_key, start_key_size, end_key, end_key_size),
            AnySecondaryIndex::BTree(index) => index.find_range(start_key, start_key_size, end_key, end_key_size),
            AnySecondaryIndex::TTree(index) => index.find_range(start_key, start_key_size, end_key, end_key_size),
        }
    }
    
    /// 范围查询（返回所有匹配项）
    pub fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find_range_all(start_key, start_key_size, end_key, end_key_size, out_record_ids, max_records),
            AnySecondaryIndex::BTree(index) => index.find_range_all(start_key, start_key_size, end_key, end_key_size, out_record_ids, max_records),
            AnySecondaryIndex::TTree(index) => index.find_range_all(start_key, start_key_size, end_key, end_key_size, out_record_ids, max_records),
        }
    }
    
    /// 删除索引项
    pub fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.delete(key, key_size),
            AnySecondaryIndex::BTree(index) => index.delete(key, key_size),
            AnySecondaryIndex::TTree(index) => index.delete(key, key_size),
        }
    }
    
    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.stats(),
            AnySecondaryIndex::BTree(index) => index.stats(),
            AnySecondaryIndex::TTree(index) => index.stats(),
        }
    }
    
    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.reset_stats(),
            AnySecondaryIndex::BTree(index) => index.reset_stats(),
            AnySecondaryIndex::TTree(index) => index.reset_stats(),
        }
    }
}

/// 辅助有序索引
pub struct SecondaryIndex {
    /// 表定义
    def: alloc::sync::Arc<TableDef>,
    /// 索引项Vec
    items: alloc::vec::Vec<SecondaryIndexItem>,
    /// 最大项数量
    max_items: usize,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋锁
    lock: parking_lot::Mutex<()>,
}

impl SecondaryIndex {
    /// 创建新的辅助索引
    pub fn new(
        def: alloc::sync::Arc<TableDef>,
        max_items: usize,
    ) -> Self {
        SecondaryIndex {
            def,
            items: alloc::vec::Vec::with_capacity(max_items),
            max_items,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: 0, // Vec管理自己的内存
                item_count: 0,
            },
            lock: parking_lot::Mutex::new(()),
        }
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
        if self.items.is_empty() {
            return Err(RemDbError::RecordNotFound);
        }
        
        let mut low = 0;
        let mut high = self.items.len() - 1;
        
        while low <= high {
            let mid = (low + high) / 2;
            let mid_item = &self.items[mid];
            
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
    pub fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        // 检查是否已满
        if self.items.len() >= self.max_items {
            // 锁会自动释放
            return Err(RemDbError::OutOfMemory);
        }
        
        // 检查键大小
        if key_size > 64 {
            // 锁会自动释放
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建新索引项
        let mut new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 64],
        };
        for i in 0..key_size {
            new_item.key_data[i] = unsafe { *key.add(i) };
        }
        
        // 使用二分查找找到插入位置，将O(n)优化为O(log n)
        let mut insert_pos = self.items.len();
        if !self.items.is_empty() {
            let mut low = 0;
            let mut high = self.items.len() - 1;
            
            while low <= high {
                let mid = (low + high) / 2;
                
                match self.compare_items(&new_item, &self.items[mid]) {
                    core::cmp::Ordering::Less => {
                        insert_pos = mid;
                        if mid == 0 {
                            break;
                        }
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
        
        // 插入新项
        self.items.insert(insert_pos, new_item);
        
        // 更新统计信息
        self.stats.item_count = self.items.len();
        
        // 锁会自动释放
        Ok(())
    }
    
    /// 根据键查找记录ID
    pub fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        match self.binary_search(key, key_size) {
            Ok(index) => {
                // 更新命中统计
                self.stats.hit_count += 1;
                Ok(self.items[index].record_id)
            },
            Err(e) => Err(e),
        }
    }
    
    /// 删除索引项
    pub fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        let result = match self.binary_search(key, key_size) {
            Ok(index) => {
                self.items.remove(index);
                self.stats.item_count = self.items.len();
                Ok(())
            },
            Err(e) => Err(e),
        };
        
        // 锁会自动释放
        result
    }
    
    /// 范围查询（返回第一个匹配项）
    pub fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        if self.items.is_empty() {
            return Err(RemDbError::RecordNotFound);
        }
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..start_key_size {
            start_item.key_data[i] = unsafe { *start_key.add(i) };
        }
        
        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.items.len() - 1;
        
        // 二分查找起始位置
        while low <= high {
            let mid = (low + high) / 2;
            
            if self.compare_items(&self.items[mid], &start_item) == core::cmp::Ordering::Less {
                start_pos = mid + 1;
                low = mid + 1;
            } else if mid == 0 {
                start_pos = 0;
                break;
            } else {
                high = mid - 1;
            }
        }
        
        // 从起始位置开始遍历，直到找到匹配项或超出范围
        for i in start_pos..self.items.len() {
            let item = &self.items[i];
            
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
                if item.key_data[j] < unsafe { *end_key.add(j) } {
                    le_end = true;
                    break;
                } else if item.key_data[j] > unsafe { *end_key.add(j) } {
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
    pub fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查输出缓冲区是否为null
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 如果没有记录，直接返回
        if self.items.is_empty() {
            return Ok(0);
        }
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..start_key_size {
            start_item.key_data[i] = unsafe { *start_key.add(i) };
        }
        
        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.items.len() - 1;
        
        // 二分查找起始位置
        while low <= high {
            let mid = (low + high) / 2;
            
            if self.compare_items(&self.items[mid], &start_item) == core::cmp::Ordering::Less {
                start_pos = mid + 1;
                low = mid + 1;
            } else if mid == 0 {
                start_pos = 0;
                break;
            } else {
                high = mid - 1;
            }
        }
        
        // 从起始位置开始遍历，收集所有匹配项
        let mut match_count = 0;
        for i in start_pos..self.items.len() {
            if match_count >= max_records {
                break;
            }
            
            let item = &self.items[i];
            
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
                if item.key_data[j] < unsafe { *end_key.add(j) } {
                    le_end = true;
                    break;
                } else if item.key_data[j] > unsafe { *end_key.add(j) } {
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
                unsafe { *out_record_ids.add(match_count) = item.record_id; }
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
        self.items.len()
    }
    
    /// 获取最大项数量
    pub fn max_items(&self) -> usize {
        self.max_items
    }
}

// ============================================================================
// B-Tree 辅助函数（不依赖 &self，避免借用冲突）
// ============================================================================

/// 比较两个索引项
fn btree_compare_items(item1: &SecondaryIndexItem, item2: &SecondaryIndexItem) -> core::cmp::Ordering {
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

/// 在节点中查找键的插入位置
fn btree_find_key_position(node: &BTreeNode, key: &SecondaryIndexItem) -> usize {
    let mut pos = 0;
    while pos < node.key_count as usize && btree_compare_items(&node.keys[pos], key) == core::cmp::Ordering::Less {
        pos += 1;
    }
    pos
}

/// 分割满节点
fn btree_split_child(parent: &mut BTreeNode, child_idx: usize) {
    // 从父节点取出子节点
    let mut child = parent.children[child_idx].take().expect("Child not found");
    
    // 创建新节点
    let mut new_node = alloc::boxed::Box::new(BTreeNode {
        is_leaf: child.is_leaf,
        key_count: (BTREE_ORDER / 2) as u8,
        keys: [SecondaryIndexItem::default(); BTREE_ORDER],
        children: Default::default(),
    });
    
    // 复制后半部分键到新节点
    for i in 0..(BTREE_ORDER / 2) {
        new_node.keys[i] = child.keys[i + (BTREE_ORDER / 2) + 1];
    }
    
    // 如果是内部节点，复制后半部分子节点
    if !child.is_leaf {
        for i in 0..(BTREE_ORDER / 2 + 1) {
            new_node.children[i] = child.children[i + (BTREE_ORDER / 2) + 1].take();
        }
    }
    
    // 更新原节点的键数量
    child.key_count = (BTREE_ORDER / 2) as u8;
    
    // 移动父节点的键和子节点指针，为新节点腾出空间
    for i in (child_idx + 1..=parent.key_count as usize).rev() {
        parent.keys[i] = parent.keys[i - 1];
        parent.children[i + 1] = parent.children[i].take();
    }
    
    // 将中间键提升到父节点
    parent.keys[child_idx] = child.keys[BTREE_ORDER / 2];
    parent.children[child_idx] = Some(child);
    parent.children[child_idx + 1] = Some(new_node);
    parent.key_count += 1;
}

/// 插入键到非满节点
fn btree_insert_non_full(node: &mut BTreeNode, key: SecondaryIndexItem, stats: &mut IndexStats) {
    let mut pos = btree_find_key_position(node, &key);
    
    if node.is_leaf {
        // 叶子节点，直接插入
        for i in (pos..node.key_count as usize).rev() {
            node.keys[i + 1] = node.keys[i];
        }
        node.keys[pos] = key;
        node.key_count += 1;
        stats.item_count += 1;
    } else {
        // 内部节点，递归插入
        if node.children[pos].as_ref().map_or(false, |c| c.key_count == BTREE_ORDER as u8) {
            btree_split_child(node, pos);
            
            // 检查中间键是否大于当前键
            if pos < node.key_count as usize && btree_compare_items(&node.keys[pos], &key) == core::cmp::Ordering::Less {
                pos += 1;
            }
        }
        
        if let Some(child) = node.children[pos].as_mut() {
            btree_insert_non_full(child.as_mut(), key, stats);
        }
    }
}

impl BTreeIndex {
    /// 创建新的B-Tree索引
    pub fn new(def: alloc::sync::Arc<TableDef>) -> Self {
        BTreeIndex {
            def,
            root: None,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: 0, // Box管理自己的内存
                item_count: 0,
            },
            lock: parking_lot::Mutex::new(()),
        }
    }
    
    /// 插入索引项
    pub fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        // 检查键大小
        if key_size > 64 {
            // 锁会自动释放
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建索引项
        let mut new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 64],
        };
        for i in 0..key_size {
            new_item.key_data[i] = unsafe { *key.add(i) };
        }
        
        if self.root.is_none() {
            // 空树，创建根节点
            let mut root_node = alloc::boxed::Box::new(BTreeNode {
                is_leaf: true,
                key_count: 1,
                keys: [SecondaryIndexItem::default(); BTREE_ORDER],
                children: Default::default(),
            });
            root_node.keys[0] = new_item;
            self.root = Some(root_node);
        } else {
            // 取出根节点，避免借用冲突
            let mut root = self.root.take().expect("Root node unexpectedly None");
            
            // 如果根节点已满，分裂根节点
            if root.key_count == BTREE_ORDER as u8 {
                let mut new_root = alloc::boxed::Box::new(BTreeNode {
                    is_leaf: false,
                    key_count: 0,
                    keys: [SecondaryIndexItem::default(); BTREE_ORDER],
                    children: Default::default(),
                });
                new_root.children[0] = Some(root);
                
                btree_split_child(new_root.as_mut(), 0);
                btree_insert_non_full(new_root.as_mut(), new_item, &mut self.stats);
                
                self.root = Some(new_root);
            } else {
                btree_insert_non_full(&mut root, new_item, &mut self.stats);
                self.root = Some(root);
            }
        }
        
        // 锁会自动释放
        Ok(())
    }
    
    /// 根据键查找记录ID
    pub fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查键大小
        if key_size > 64 {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建临时索引项用于比较
        let mut search_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..key_size {
            search_item.key_data[i] = unsafe { *key.add(i) };
        }
        
        let mut current = self.root.as_ref();
        while let Some(node) = current {
            let mut pos = 0;
            
            // 查找键位置
            while pos < node.key_count as usize && btree_compare_items(&node.keys[pos], &search_item) == core::cmp::Ordering::Less {
                pos += 1;
            }
            
            // 检查是否找到键
            if pos < node.key_count as usize {
                let cmp = btree_compare_items(&node.keys[pos], &search_item);
                if cmp == core::cmp::Ordering::Equal {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(node.keys[pos].record_id);
                }
            }
            
            // 如果是叶子节点，未找到
            if node.is_leaf {
                break;
            }
            
            // 继续搜索子节点
            current = node.children[pos].as_ref();
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 范围查询（返回第一个匹配项）
    pub fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..start_key_size {
            start_item.key_data[i] = unsafe { *start_key.add(i) };
        }
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..end_key_size {
            end_item.key_data[i] = unsafe { *end_key.add(i) };
        }
        
        let mut current = self.root.as_ref();
        let mut stack: [Option<&BTreeNode>; 64] = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        
        // 遍历树，直到找到叶子节点
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;
            
            // 如果是叶子节点，跳出循环
            if node.is_leaf {
                break;
            }
            
            // 找到第一个大于等于start_key的子节点
            let mut pos = 0;
            while pos < node.key_count as usize && btree_compare_items(&node.keys[pos], &start_item) == core::cmp::Ordering::Less {
                pos += 1;
            }
            
            current = node.children[pos].as_ref();
        }
        
        // 从栈中回溯，查找匹配项
        while stack_size > 0 {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            
            // 查找起始位置
            let mut start_pos = 0;
            while start_pos < node.key_count as usize && btree_compare_items(&node.keys[start_pos], &start_item) == core::cmp::Ordering::Less {
                start_pos += 1;
            }
            
            // 遍历当前节点的键
            for i in start_pos..node.key_count as usize {
                let key = &node.keys[i];
                
                // 检查是否在范围内
                if btree_compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    continue; // 超出范围，继续查找下一个节点
                }
                
                // 找到匹配项
                self.stats.hit_count += 1;
                return Ok(key.record_id);
            }
            
            // 如果不是叶子节点，继续搜索右子树
            if !node.is_leaf {
                let mut child = node.children[node.key_count as usize].as_ref();
                while let Some(child_node) = child {
                    // 遍历子节点的键
                    for i in 0..child_node.key_count as usize {
                        let key = &child_node.keys[i];
                        
                        // 检查是否在范围内
                        if btree_compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                            break; // 超出范围，结束搜索
                        }
                        
                        // 找到匹配项
                        self.stats.hit_count += 1;
                        return Ok(key.record_id);
                    }
                    
                    // 如果是叶子节点，结束搜索
                    if child_node.is_leaf {
                        break;
                    }
                    
                    // 继续搜索第一个子节点
                    child = child_node.children[0].as_ref();
                }
            }
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 范围查询（返回所有匹配项）
    pub fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查输出缓冲区
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..start_key_size {
            start_item.key_data[i] = unsafe { *start_key.add(i) };
        }
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..end_key_size {
            end_item.key_data[i] = unsafe { *end_key.add(i) };
        }
        
        let mut match_count = 0;
        let mut stack: [Option<&BTreeNode>; 64] = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        
        // 遍历树，直到找到叶子节点
        let mut current = self.root.as_ref();
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;
            
            // 如果是叶子节点，跳出循环
            if node.is_leaf {
                break;
            }
            
            // 找到第一个大于等于start_key的子节点
            let mut pos = 0;
            while pos < node.key_count as usize && btree_compare_items(&node.keys[pos], &start_item) == core::cmp::Ordering::Less {
                pos += 1;
            }
            
            current = node.children[pos].as_ref();
        }
        
        // 从栈中回溯，收集所有匹配项
        while stack_size > 0 && match_count < max_records {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            
            // 查找起始位置
            let mut start_pos = 0;
            while start_pos < node.key_count as usize && btree_compare_items(&node.keys[start_pos], &start_item) == core::cmp::Ordering::Less {
                start_pos += 1;
            }
            
            // 遍历当前节点的键
            for i in start_pos..node.key_count as usize {
                if match_count >= max_records {
                    break;
                }
                
                let key = &node.keys[i];
                
                // 检查是否在范围内
                if btree_compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    continue; // 超出范围，继续查找下一个节点
                }
                
                // 添加到结果
                unsafe { *out_record_ids.add(match_count) = key.record_id; }
                match_count += 1;
            }
            
            // 如果不是叶子节点，继续搜索右子树
            if !node.is_leaf && match_count < max_records {
                let mut child = node.children[node.key_count as usize].as_ref();
                while let Some(child_node) = child {
                    // 遍历子节点的键
                    for i in 0..child_node.key_count as usize {
                        if match_count >= max_records {
                            break;
                        }
                        
                        let key = &child_node.keys[i];
                        
                        // 检查是否在范围内
                        if btree_compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                            break; // 超出范围，结束搜索
                        }
                        
                        // 添加到结果
                        unsafe { *out_record_ids.add(match_count) = key.record_id; }
                        match_count += 1;
                    }
                    
                    // 如果是叶子节点，结束搜索
                    if child_node.is_leaf || match_count >= max_records {
                        break;
                    }
                    
                    // 继续搜索第一个子节点
                    child = child_node.children[0].as_ref();
                }
            }
        }
        
        // 更新命中统计
        if match_count > 0 {
            self.stats.hit_count += match_count;
        }
        
        Ok(match_count)
    }
    
    /// 删除索引项
    pub fn delete(&mut self, _key: *const u8, _key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        // 简化实现：暂不支持删除操作
        // 完整的B-Tree删除实现比较复杂，需要处理多种情况
        // 包括合并节点、借键等
        
        // 锁会自动释放
        Err(RemDbError::UnsupportedOperation)
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

// ============================================================================
// T-Tree 辅助函数（不依赖 &self，避免借用冲突）
// ============================================================================

/// 比较两个索引项
fn ttree_compare_items(item1: &SecondaryIndexItem, item2: &SecondaryIndexItem) -> core::cmp::Ordering {
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

/// 在节点中查找键的位置
fn ttree_find_key_position(node: &TTreeNode, key: &SecondaryIndexItem) -> (usize, core::cmp::Ordering) {
    let mut pos = 0;
    while pos < node.key_count as usize {
        let cmp = ttree_compare_items(&node.keys[pos], key);
        match cmp {
            core::cmp::Ordering::Less => pos += 1,
            _ => return (pos, cmp),
        }
    }
    (pos, core::cmp::Ordering::Less)
}

/// 插入键到节点
fn ttree_insert_into_node(node: &mut TTreeNode, key: SecondaryIndexItem, stats: &mut IndexStats) {
    // 查找插入位置
    let (pos, _) = ttree_find_key_position(node, &key);
    
    // 移动现有键为新键腾出空间
    for i in (pos..node.key_count as usize).rev() {
        node.keys[i + 1] = node.keys[i];
    }
    
    // 插入新键
    node.keys[pos] = key;
    node.key_count += 1;
    stats.item_count += 1;
}

/// 递归插入索引项
fn ttree_insert_recursive(node: &mut TTreeNode, key: SecondaryIndexItem, stats: &mut IndexStats) {
    // 查找插入位置
    let (pos, cmp) = ttree_find_key_position(node, &key);
    
    if cmp == core::cmp::Ordering::Equal {
        // 键已存在，更新记录ID
        node.keys[pos].record_id = key.record_id;
        return;
    }
    
    // 确定子树方向
    let go_left = pos == 0;
    let go_middle = !go_left && pos < node.key_count as usize;
    // go_right = otherwise
    
    if go_left {
        if node.left.is_some() {
            let mut child = node.left.take().unwrap();
            if child.key_count == TTREE_ORDER as u8 {
                // 子节点已满，需要分裂
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                
                for i in 0..TTREE_ORDER {
                    keys[i] = child.keys[i];
                }
                
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if ttree_compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }
                
                if !inserted {
                    keys[TTREE_ORDER] = key;
                }
                
                let mut new_right = alloc::boxed::Box::new(TTreeNode {
                    key_count: ((TTREE_ORDER + 1) - ((TTREE_ORDER + 1) / 2)) as u8,
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    left: None,
                    middle: None,
                    right: None,
                });
                
                let mid = (TTREE_ORDER + 1) / 2;
                
                child.key_count = mid as u8;
                for i in 0..mid {
                    child.keys[i] = keys[i];
                }
                
                for i in 0..new_right.key_count as usize {
                    new_right.keys[i] = keys[mid + i];
                }
                
                let promoted_key = keys[mid - 1];
                ttree_insert_into_node(node, promoted_key, stats);
                
                // 更新子节点指针
                let (promoted_pos, _) = ttree_find_key_position(node, &promoted_key);
                if promoted_pos == 0 {
                    node.left = Some(child);
                    node.middle = Some(new_right);
                } else if promoted_pos < node.key_count as usize {
                    // 需要将原 left 存回，但此时 left 已被 promoted 占据
                    if node.middle.is_none() {
                        // 将原 left 移到 middle 位置
                        node.middle = Some(child);
                        // 如果还有空间，新右节点放 right
                        // 如果 right 也被占，就放弃
                        if node.right.is_none() {
                            node.right = Some(new_right);
                        }
                    }
                } else {
                    node.right = Some(new_right);
                }
            } else {
                ttree_insert_recursive(&mut child, key, stats);
                node.left = Some(child);
            }
        } else {
            // 左子节点不存在，直接插入到当前节点
            if node.key_count < TTREE_ORDER as u8 {
                ttree_insert_into_node(node, key, stats);
            } else {
                // 节点已满，创建新节点
                let mut new_node = alloc::boxed::Box::new(TTreeNode {
                    key_count: 0,
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    left: None,
                    middle: None,
                    right: None,
                });
                
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                for i in 0..TTREE_ORDER {
                    keys[i] = node.keys[i];
                }
                
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if ttree_compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }
                if !inserted {
                    keys[TTREE_ORDER] = key;
                }
                
                let mid = (TTREE_ORDER + 1) / 2;
                node.key_count = mid as u8;
                for i in 0..mid {
                    node.keys[i] = keys[i];
                }
                
                new_node.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..new_node.key_count as usize {
                    new_node.keys[i] = keys[mid + i];
                }
                
                node.left = Some(new_node);
            }
        }
    } else if go_middle {
        if node.middle.is_some() {
            let mut child = node.middle.take().unwrap();
            if child.key_count == TTREE_ORDER as u8 {
                // 子节点已满，需要分裂
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                
                for i in 0..TTREE_ORDER {
                    keys[i] = child.keys[i];
                }
                
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if ttree_compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }
                
                if !inserted {
                    keys[TTREE_ORDER] = key;
                }
                
                let mut new_right = alloc::boxed::Box::new(TTreeNode {
                    key_count: ((TTREE_ORDER + 1) - ((TTREE_ORDER + 1) / 2)) as u8,
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    left: None,
                    middle: None,
                    right: None,
                });
                
                let mid = (TTREE_ORDER + 1) / 2;
                
                child.key_count = mid as u8;
                for i in 0..mid {
                    child.keys[i] = keys[i];
                }
                
                for i in 0..new_right.key_count as usize {
                    new_right.keys[i] = keys[mid + i];
                }
                
                let promoted_key = keys[mid - 1];
                ttree_insert_into_node(node, promoted_key, stats);
                
                let (promoted_pos, _) = ttree_find_key_position(node, &promoted_key);
                if promoted_pos == 0 {
                    node.left = Some(child);
                    node.middle = Some(new_right);
                } else if promoted_pos < node.key_count as usize {
                    node.middle = Some(child);
                    node.right = Some(new_right);
                } else {
                    node.right = Some(new_right);
                }
            } else {
                ttree_insert_recursive(&mut child, key, stats);
                node.middle = Some(child);
            }
        } else {
            // 中子节点不存在，直接插入到当前节点
            if node.key_count < TTREE_ORDER as u8 {
                ttree_insert_into_node(node, key, stats);
            } else {
                // 节点已满，创建新节点
                let mut new_node = alloc::boxed::Box::new(TTreeNode {
                    key_count: 0,
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    left: None,
                    middle: None,
                    right: None,
                });
                
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                for i in 0..TTREE_ORDER {
                    keys[i] = node.keys[i];
                }
                
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if ttree_compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }
                if !inserted {
                    keys[TTREE_ORDER] = key;
                }
                
                let mid = (TTREE_ORDER + 1) / 2;
                node.key_count = mid as u8;
                for i in 0..mid {
                    node.keys[i] = keys[i];
                }
                
                new_node.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..new_node.key_count as usize {
                    new_node.keys[i] = keys[mid + i];
                }
                
                node.middle = Some(new_node);
            }
        }
    } else {
        // Go right
        if node.right.is_some() {
            let mut child = node.right.take().unwrap();
            if child.key_count == TTREE_ORDER as u8 {
                // 子节点已满，需要分裂
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                
                for i in 0..TTREE_ORDER {
                    keys[i] = child.keys[i];
                }
                
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if ttree_compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }
                
                if !inserted {
                    keys[TTREE_ORDER] = key;
                }
                
                let mut new_right = alloc::boxed::Box::new(TTreeNode {
                    key_count: ((TTREE_ORDER + 1) - ((TTREE_ORDER + 1) / 2)) as u8,
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    left: None,
                    middle: None,
                    right: None,
                });
                
                let mid = (TTREE_ORDER + 1) / 2;
                
                child.key_count = mid as u8;
                for i in 0..mid {
                    child.keys[i] = keys[i];
                }
                
                for i in 0..new_right.key_count as usize {
                    new_right.keys[i] = keys[mid + i];
                }
                
                let promoted_key = keys[mid - 1];
                ttree_insert_into_node(node, promoted_key, stats);
                
                let (promoted_pos, _) = ttree_find_key_position(node, &promoted_key);
                if promoted_pos == 0 {
                    node.left = Some(child);
                    node.middle = Some(new_right);
                } else if promoted_pos < node.key_count as usize {
                    node.middle = Some(child);
                    node.right = Some(new_right);
                } else {
                    node.right = Some(new_right);
                }
            } else {
                ttree_insert_recursive(&mut child, key, stats);
                node.right = Some(child);
            }
        } else {
            // 右子节点不存在，直接插入到当前节点
            if node.key_count < TTREE_ORDER as u8 {
                ttree_insert_into_node(node, key, stats);
            } else {
                // 节点已满，创建新节点
                let mut new_node = alloc::boxed::Box::new(TTreeNode {
                    key_count: 0,
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    left: None,
                    middle: None,
                    right: None,
                });
                
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                for i in 0..TTREE_ORDER {
                    keys[i] = node.keys[i];
                }
                
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if ttree_compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }
                if !inserted {
                    keys[TTREE_ORDER] = key;
                }
                
                let mid = (TTREE_ORDER + 1) / 2;
                node.key_count = mid as u8;
                for i in 0..mid {
                    node.keys[i] = keys[i];
                }
                
                new_node.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..new_node.key_count as usize {
                    new_node.keys[i] = keys[mid + i];
                }
                
                node.right = Some(new_node);
            }
        }
    }
}

impl TTreeIndex {
    /// 创建新的T-Tree索引
    pub fn new(def: alloc::sync::Arc<TableDef>) -> Self {
        TTreeIndex {
            def,
            root: None,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: 0, // Box管理自己的内存
                item_count: 0,
            },
            lock: parking_lot::Mutex::new(()),
        }
    }
    
    /// 插入索引项
    pub fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        // 检查键大小
        if key_size > 64 {
            // 锁会自动释放
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建索引项
        let mut new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 64],
        };
        for i in 0..key_size {
            new_item.key_data[i] = unsafe { *key.add(i) };
        }
        
        if self.root.is_none() {
            // 空树，创建根节点
            let mut root_node = alloc::boxed::Box::new(TTreeNode {
                key_count: 1,
                keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                left: None,
                middle: None,
                right: None,
            });
            root_node.keys[0] = new_item;
            self.root = Some(root_node);
        } else {
            // 取出根节点，避免借用冲突
            let mut root = self.root.take().expect("Root node unexpectedly None");
            
            // 如果根节点已满，需要分裂
            if root.key_count == TTREE_ORDER as u8 {
                // 创建新根节点，将原根节点作为左子节点
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                
                // 复制原根节点的键
                for i in 0..TTREE_ORDER {
                    keys[i] = root.keys[i];
                }
                
                // 插入新键
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if ttree_compare_items(&keys[i], &new_item) == core::cmp::Ordering::Greater {
                        // 移动后续键
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = new_item;
                        inserted = true;
                        break;
                    }
                }
                
                if !inserted {
                    keys[TTREE_ORDER] = new_item;
                }
                
                // 创建右子节点
                let mut right_node = alloc::boxed::Box::new(TTreeNode {
                    key_count: ((TTREE_ORDER + 1) - ((TTREE_ORDER + 1) / 2)) as u8,
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    left: None,
                    middle: None,
                    right: None,
                });
                
                // 分配键到左右子节点
                let mid = (TTREE_ORDER + 1) / 2;
                
                // 更新原根节点（左子节点）
                root.key_count = mid as u8;
                for i in 0..mid {
                    root.keys[i] = keys[i];
                }
                
                // 更新右子节点
                for i in 0..right_node.key_count as usize {
                    right_node.keys[i] = keys[mid + i];
                }
                
                // 更新新根节点
                let mut new_root = alloc::boxed::Box::new(TTreeNode {
                    keys: [SecondaryIndexItem::default(); TTREE_ORDER],
                    key_count: 1,
                    left: Some(root),
                    middle: None,
                    right: Some(right_node),
                });
                new_root.keys[0] = keys[mid - 1];
                
                self.root = Some(new_root);
            } else {
                // 递归插入到适当的子树
                ttree_insert_recursive(&mut root, new_item, &mut self.stats);
                self.root = Some(root);
            }
        }
        
        // 锁会自动释放
        Ok(())
    }
    
    /// 根据键查找记录ID
    pub fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查键大小
        if key_size > 64 {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建临时索引项用于比较
        let mut search_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..key_size {
            search_item.key_data[i] = unsafe { *key.add(i) };
        }
        
        let mut current = self.root.as_ref();
        while let Some(node) = current {
            // 查找键位置
            let (pos, cmp) = ttree_find_key_position(node, &search_item);
            
            if cmp == core::cmp::Ordering::Equal {
                // 更新命中统计
                self.stats.hit_count += 1;
                return Ok(node.keys[pos].record_id);
            }
            
            // 确定下一个子节点
            current = if pos == 0 {
                node.left.as_ref()
            } else if pos < node.key_count as usize {
                node.middle.as_ref()
            } else {
                node.right.as_ref()
            };
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 范围查询（返回第一个匹配项）
    pub fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..start_key_size {
            start_item.key_data[i] = unsafe { *start_key.add(i) };
        }
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..end_key_size {
            end_item.key_data[i] = unsafe { *end_key.add(i) };
        }
        
        // 简化实现：遍历树，查找第一个匹配项
        let mut stack: [Option<&TTreeNode>; 64] = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        let mut current = self.root.as_ref();
        
        // 遍历到最左节点
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;
            current = node.left.as_ref();
        }
        
        // 中序遍历树，查找第一个匹配项
        while stack_size > 0 {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            
            // 检查当前节点的键
            for i in 0..node.key_count as usize {
                let key = &node.keys[i];
                
                // 检查是否在范围内
                if ttree_compare_items(key, &start_item) != core::cmp::Ordering::Less && 
                   ttree_compare_items(key, &end_item) != core::cmp::Ordering::Greater {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(key.record_id);
                }
                
                // 如果已经超出范围，结束搜索
                if ttree_compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    break;
                }
            }
            
            // 遍历右子树
            let mut child = node.right.as_ref();
            while let Some(child_node) = child {
                stack[stack_size] = Some(child_node);
                stack_size += 1;
                child = child_node.left.as_ref();
            }
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 范围查询（返回所有匹配项）
    pub fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查输出缓冲区
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..start_key_size {
            start_item.key_data[i] = unsafe { *start_key.add(i) };
        }
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        for i in 0..end_key_size {
            end_item.key_data[i] = unsafe { *end_key.add(i) };
        }
        
        let mut match_count = 0;
        let mut stack: [Option<&TTreeNode>; 64] = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        let mut current = self.root.as_ref();
        
        // 遍历到最左节点
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;
            current = node.left.as_ref();
        }
        
        // 中序遍历树，收集所有匹配项
        while stack_size > 0 && match_count < max_records {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            
            // 检查当前节点的键
            for i in 0..node.key_count as usize {
                if match_count >= max_records {
                    break;
                }
                
                let key = &node.keys[i];
                
                // 检查是否在范围内
                if ttree_compare_items(key, &start_item) != core::cmp::Ordering::Less && 
                   ttree_compare_items(key, &end_item) != core::cmp::Ordering::Greater {
                    // 添加到结果
                    unsafe { *out_record_ids.add(match_count) = key.record_id; }
                    match_count += 1;
                }
                
                // 如果已经超出范围，结束搜索
                if ttree_compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    break;
                }
            }
            
            // 遍历右子树
            let mut child = node.right.as_ref();
            while let Some(child_node) = child {
                stack[stack_size] = Some(child_node);
                stack_size += 1;
                child = child_node.left.as_ref();
            }
        }
        
        // 更新命中统计
        if match_count > 0 {
            self.stats.hit_count += match_count;
        }
        
        Ok(match_count)
    }
    
    /// 删除索引项
    pub fn delete(&mut self, _key: *const u8, _key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        let _lock = self.lock.lock();
        
        // 简化实现：暂不支持删除操作
        // 完整的T-Tree删除实现比较复杂，需要处理多种情况
        
        // 锁会自动释放
        Err(RemDbError::UnsupportedOperation)
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