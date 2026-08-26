use crate::types::Result;
use core::ptr::NonNull;

/// 固定大小内存池
pub struct MemoryPool {
    /// 内存池起始地址
    start_ptr: NonNull<u8>,
    /// 内存块大小
    block_size: usize,
    /// 总块数
    total_blocks: usize,
    /// 已使用块数
    used_blocks: usize,
    /// 空闲列表头
    free_list: Option<NonNull<u8>>,
}

impl MemoryPool {
    /// 创建新的内存池
    pub unsafe fn new(start_ptr: *mut u8, block_size: usize, total_blocks: usize) -> Self {
        let start_ptr = NonNull::new_unchecked(start_ptr);
        let aligned_block_size = (block_size + 7) & !7; // 8字节对齐

        // 初始化空闲列表
        let mut free_list = None;
        let mut current_ptr = start_ptr;

        for _ in 0..total_blocks {
            let block_ptr = current_ptr;
            current_ptr = NonNull::new_unchecked(
                (current_ptr.as_ptr() as usize + aligned_block_size) as *mut u8,
            );
            let next_ptr = free_list;
            core::ptr::write(block_ptr.as_ptr() as *mut Option<NonNull<u8>>, next_ptr);
            free_list = Some(block_ptr);
        }

        MemoryPool {
            start_ptr,
            block_size: aligned_block_size,
            total_blocks,
            used_blocks: 0,
            free_list,
        }
    }

    /// 分配一个内存块
    pub unsafe fn allocate(&mut self) -> Result<NonNull<u8>> {
        if let Some(block_ptr) = self.free_list {
            let next_ptr = core::ptr::read(block_ptr.as_ptr() as *const Option<NonNull<u8>>);
            self.free_list = next_ptr;
            self.used_blocks += 1;
            Ok(block_ptr)
        } else {
            Err(crate::types::RemDbError::OutOfMemory)
        }
    }

    /// 释放一个内存块
    pub unsafe fn free(&mut self, ptr: NonNull<u8>) {
        let ptr_addr = ptr.as_ptr() as usize;
        let start_addr = self.start_ptr.as_ptr() as usize;
        let end_addr = start_addr + self.block_size * self.total_blocks;

        if ptr_addr >= start_addr && ptr_addr < end_addr {
            core::ptr::write(ptr.as_ptr() as *mut Option<NonNull<u8>>, self.free_list);
            self.free_list = Some(ptr);
            self.used_blocks -= 1;
        }
    }

    /// 获取已使用块数
    pub fn used_blocks(&self) -> usize {
        self.used_blocks
    }

    /// 获取总块数
    pub fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    /// 获取内存池使用率
    pub fn usage(&self) -> f32 {
        if self.total_blocks == 0 {
            0.0
        } else {
            self.used_blocks as f32 / self.total_blocks as f32
        }
    }

    /// 检查指针是否在内存池范围内
    pub fn contains(&self, ptr: NonNull<u8>) -> bool {
        let ptr_addr = ptr.as_ptr() as usize;
        let start_addr = self.start_ptr.as_ptr() as usize;
        let end_addr = start_addr + self.block_size * self.total_blocks;
        ptr_addr >= start_addr && ptr_addr < end_addr
    }
}

/// 多内存池管理器
pub struct MultiPoolManager<'a> {
    pools: &'a mut [MemoryPool],
    pool_count: usize,
}

impl<'a> MultiPoolManager<'a> {
    pub unsafe fn new(pools: &'a mut [MemoryPool]) -> Self {
        let pool_count = pools.len();
        MultiPoolManager { pools, pool_count }
    }

    pub unsafe fn allocate(&mut self, size: usize) -> Result<NonNull<u8>> {
        for pool in &mut self.pools[..self.pool_count] {
            if pool.block_size >= size {
                return pool.allocate();
            }
        }
        Err(crate::types::RemDbError::OutOfMemory)
    }

    pub unsafe fn free(&mut self, ptr: NonNull<u8>) -> Result<()> {
        for pool in &mut self.pools[..self.pool_count] {
            if pool.contains(ptr) {
                pool.free(ptr);
                return Ok(());
            }
        }
        Err(crate::types::RemDbError::InvalidPointer)
    }

    pub fn total_usage(&self) -> f32 {
        let mut total_used = 0;
        let mut total_blocks = 0;
        for pool in &self.pools[..self.pool_count] {
            total_used += pool.used_blocks();
            total_blocks += pool.total_blocks();
        }
        if total_blocks == 0 {
            0.0
        } else {
            total_used as f32 / total_blocks as f32
        }
    }
}

/// Slab 分配器：用于固定大小对象的快速分配
pub struct SlabAllocator {
    /// 每个槽位的大小
    slot_size: usize,
    /// 对齐要求
    alignment: usize,
    /// 总槽位数
    total_slots: usize,
    /// 已使用槽位数
    used_slots: usize,
    /// 空闲列表
    free_list: Option<NonNull<u8>>,
    /// 内存起始地址
    start_ptr: NonNull<u8>,
}

impl SlabAllocator {
    /// 创建新的 Slab 分配器
    pub unsafe fn new(
        start_ptr: *mut u8,
        slot_size: usize,
        alignment: usize,
        total_slots: usize,
    ) -> Result<Self> {
        let start_ptr = NonNull::new(start_ptr).ok_or(crate::types::RemDbError::InvalidPointer)?;
        let aligned_slot = (slot_size + alignment - 1) & !(alignment - 1);

        let mut free_list = None;
        let mut current = start_ptr;

        for _ in 0..total_slots {
            let slot_ptr = current;
            let next = (current.as_ptr() as usize + aligned_slot) as *mut u8;
            current = NonNull::new_unchecked(next);
            core::ptr::write(slot_ptr.as_ptr() as *mut Option<NonNull<u8>>, free_list);
            free_list = Some(slot_ptr);
        }

        Ok(SlabAllocator {
            slot_size: aligned_slot,
            alignment,
            total_slots,
            used_slots: 0,
            free_list,
            start_ptr,
        })
    }

    /// 分配一个槽位
    pub unsafe fn allocate(&mut self) -> Result<NonNull<u8>> {
        match self.free_list {
            Some(ptr) => {
                let next = core::ptr::read(ptr.as_ptr() as *const Option<NonNull<u8>>);
                self.free_list = next;
                self.used_slots += 1;
                Ok(ptr)
            }
            None => Err(crate::types::RemDbError::OutOfMemory),
        }
    }

    /// 释放一个槽位
    pub unsafe fn free(&mut self, ptr: NonNull<u8>) {
        if self.contains(ptr) {
            core::ptr::write(ptr.as_ptr() as *mut Option<NonNull<u8>>, self.free_list);
            self.free_list = Some(ptr);
            self.used_slots -= 1;
        }
    }

    /// 检查指针是否属于此分配器
    pub fn contains(&self, ptr: NonNull<u8>) -> bool {
        let addr = ptr.as_ptr() as usize;
        let start = self.start_ptr.as_ptr() as usize;
        let end = start + self.slot_size * self.total_slots;
        addr >= start && addr < end
    }

    /// 使用率
    pub fn usage(&self) -> f32 {
        if self.total_slots == 0 {
            0.0
        } else {
            self.used_slots as f32 / self.total_slots as f32
        }
    }

    /// 剩余槽位数
    pub fn remaining(&self) -> usize {
        self.total_slots - self.used_slots
    }
}

/// 懒加载资源包装器
pub struct LazyLoad<T, F: FnOnce() -> Result<T>> {
    data: Option<T>,
    init: Option<F>,
}

impl<T, F: FnOnce() -> Result<T>> LazyLoad<T, F> {
    pub fn new(init: F) -> Self {
        LazyLoad {
            data: None,
            init: Some(init),
        }
    }

    pub fn get(&mut self) -> Result<&T> {
        if self.data.is_none() {
            if let Some(init_fn) = self.init.take() {
                self.data = Some(init_fn()?);
            }
        }
        match &self.data {
            Some(data) => Ok(data),
            None => Err(crate::types::RemDbError::UnsupportedOperation),
        }
    }

    pub fn get_mut(&mut self) -> Result<&mut T> {
        if self.data.is_none() {
            if let Some(init_fn) = self.init.take() {
                self.data = Some(init_fn()?);
            }
        }
        match &mut self.data {
            Some(data) => Ok(data),
            None => Err(crate::types::RemDbError::UnsupportedOperation),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }

    pub fn drop_data(&mut self) {
        self.data = None;
    }
}