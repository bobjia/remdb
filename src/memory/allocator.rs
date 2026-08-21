//! 内存分配器封装层。
//!
//! 转发 `remdb-alloc` crate 提供的全局分配器接口，供事务日志恢复等
//! 底层路径使用。

use core::ptr::NonNull;

pub use remdb_alloc::{
    init_global_allocator, reset_allocator, MemoryBlock, MemoryStats, StaticAllocator,
};

/// 分配一块内存（未初始化），失败时返回人类可读的错误信息。
pub fn alloc(size: usize) -> Result<NonNull<u8>, &'static str> {
    remdb_alloc::alloc(size)
}

/// 释放一块由 [`alloc`] 分配的内存。
pub fn free(ptr: NonNull<u8>) -> Result<(), &'static str> {
    remdb_alloc::free(ptr)
}