use alloc::vec::Vec;
use crate::types::Result;

/// 固定大小内存池
pub struct MemoryPool {
    /// 存储空间
    storage: Vec<u8>,
    /// 内存块大小
    block_size: usize,
    /// 总块数
    total_blocks: usize,
    /// 已使用块数
    used_blocks: usize,
    /// 空闲索引列表
    free_indices: Vec<usize>,
}

impl MemoryPool {
    /// 创建新的内存池，分配 block_size * total_blocks 字节的存储空间
    pub fn new(block_size: usize, total_blocks: usize) -> Self {
        let storage = alloc::vec![0u8; block_size * total_blocks];
        // 初始化空闲索引列表，从大到小排列以便 pop 时获取最小索引
        let mut free_indices: Vec<usize> = (0..total_blocks).collect();
        free_indices.reverse();
        MemoryPool {
            storage,
            block_size,
            total_blocks,
            used_blocks: 0,
            free_indices,
        }
    }

    /// 分配一个内存块，返回块索引
    pub fn allocate(&mut self) -> Result<usize> {
        let index = self.free_indices.pop().ok_or(crate::types::RemDbError::OutOfMemory)?;
        self.used_blocks += 1;
        Ok(index)
    }

    /// 释放指定索引的内存块
    pub fn free(&mut self, index: usize) {
        self.free_indices.push(index);
        self.used_blocks -= 1;
    }

    /// 获取指定索引块的不可变切片
    pub fn get_block(&self, index: usize) -> &[u8] {
        let start = index * self.block_size;
        &self.storage[start..start + self.block_size]
    }

    /// 获取指定索引块的可变切片
    pub fn get_block_mut(&mut self, index: usize) -> &mut [u8] {
        let start = index * self.block_size;
        &mut self.storage[start..start + self.block_size]
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

    /// 获取空闲块数
    pub fn free_blocks(&self) -> usize {
        self.total_blocks - self.used_blocks
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
    pub fn new(pools: &'a mut [MemoryPool]) -> Self {
        let pool_count = pools.len();
        MultiPoolManager {
            pools,
            pool_count,
        }
    }

    /// 根据大小分配内存块，返回 (池索引, 块索引)
    pub fn allocate(&mut self, size: usize) -> Result<(usize, usize)> {
        for (i, pool) in self.pools[..self.pool_count].iter_mut().enumerate() {
            if pool.block_size >= size {
                let block_idx = pool.allocate()?;
                return Ok((i, block_idx));
            }
        }
        Err(crate::types::RemDbError::OutOfMemory)
    }

    /// 释放指定池中指定索引的内存块
    pub fn free(&mut self, pool_index: usize, block_index: usize) {
        self.pools[pool_index].free(block_index);
    }

    /// 获取指定池中指定索引块的不可变切片
    pub fn get_block(&self, pool_index: usize, block_index: usize) -> &[u8] {
        self.pools[pool_index].get_block(block_index)
    }

    /// 获取指定池中指定索引块的可变切片
    pub fn get_block_mut(&mut self, pool_index: usize, block_index: usize) -> &mut [u8] {
        self.pools[pool_index].get_block_mut(block_index)
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