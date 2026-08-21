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
            // 保存当前块指针
            let block_ptr = current_ptr;

            // 移动到下一个块
            current_ptr = NonNull::new_unchecked(
                (current_ptr.as_ptr() as usize + aligned_block_size) as *mut u8,
            );

            // 将当前块添加到空闲列表
            let next_ptr = free_list;
            // 存储下一个块的指针在当前块的开头
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
            // 从空闲列表获取块
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
        // 确保指针在内存池范围内
        let ptr_addr = ptr.as_ptr() as usize;
        let start_addr = self.start_ptr.as_ptr() as usize;
        let end_addr = start_addr + self.block_size * self.total_blocks;

        assert!(
            ptr_addr >= start_addr && ptr_addr < end_addr,
            "Pointer not in memory pool"
        );

        // 将块添加到空闲列表
        core::ptr::write(ptr.as_ptr() as *mut Option<NonNull<u8>>, self.free_list);
        self.free_list = Some(ptr);
        self.used_blocks -= 1;
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
        self.used_blocks as f32 / self.total_blocks as f32
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
    /// 内存池列表
    pools: &'a mut [MemoryPool],
    /// 池数量
    pool_count: usize,
}

impl<'a> MultiPoolManager<'a> {
    /// 创建新的多内存池管理器
    pub unsafe fn new(pools: &'a mut [MemoryPool]) -> Self {
        // 计算pool_count并立即使用，避免同时借用
        let pool_count = pools.len();
        MultiPoolManager { pools, pool_count }
    }

    /// 根据大小分配内存块
    pub unsafe fn allocate(&mut self, size: usize) -> Result<NonNull<u8>> {
        // 找到合适大小的内存池
        for pool in &mut self.pools[..self.pool_count] {
            if pool.block_size >= size {
                return pool.allocate();
            }
        }

        Err(crate::types::RemDbError::OutOfMemory)
    }

    /// 释放内存块
    pub unsafe fn free(&mut self, ptr: NonNull<u8>) -> Result<(), crate::types::RemDbError> {
        // 找到包含该指针的内存池
        for pool in &mut self.pools[..self.pool_count] {
            if pool.contains(ptr) {
                pool.free(ptr);
                return Ok(());
            }
        }

        // 如果没有找到，返回错误
        Err(crate::types::RemDbError::InvalidPointer)
    }

    /// 获取总内存使用情况
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
