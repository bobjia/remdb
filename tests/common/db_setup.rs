//! 数据库初始化辅助函数
//!
//! 提供统一的数据库初始化和清理函数

use remdb::*;
use serial_test::serial;
use super::platform::TEST_PLATFORM;

const DEFAULT_TEST_MEMORY_SIZE: usize = 4 * 1024 * 1024; // 4MB

pub fn setup_test_db_with_memory(size: usize) -> Vec<u8> {
    let mut db_memory = Vec::with_capacity(size);
    db_memory.resize(size, 0u8);
    
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    unsafe {
        remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
            .unwrap();
    }
    
    remdb::reset_global_db();
    
    db_memory
}

pub fn setup_test_db() -> Vec<u8> {
    setup_test_db_with_memory(DEFAULT_TEST_MEMORY_SIZE)
}

pub fn cleanup_test_db() {
    remdb::reset_global_db();
}

#[cfg(feature = "posix")]
pub fn setup_test_db_with_posix_platform(size: usize) -> Vec<u8> {
    use remdb::platform::posix;
    
    let mut db_memory = Vec::with_capacity(size);
    db_memory.resize(size, 0u8);
    
    remdb::platform::init_platform(posix::get_posix_platform());
    
    unsafe {
        remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
            .unwrap();
    }
    
    remdb::reset_global_db();
    
    db_memory
}

#[cfg(feature = "posix")]
pub fn setup_test_db_with_posix() -> Vec<u8> {
    setup_test_db_with_posix_platform(DEFAULT_TEST_MEMORY_SIZE)
}
