//! 数据库初始化辅助函数
//!
//! 提供统一的数据库初始化和清理函数

use remdb::*;
use serial_test::serial;
use super::platform::TEST_PLATFORM;

const DEFAULT_TEST_MEMORY_SIZE: usize = 1024 * 1024; // 1MB

pub fn setup_test_db_with_memory(size: usize) -> Box<[u8]> {
    // 分配内存并泄漏它，确保内存在整个测试期间保持有效
    let mut db_memory = vec![0u8; size].into_boxed_slice();
    
    // 获取原始指针，但不放弃所有权
    let ptr = db_memory.as_mut_ptr();
    
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    unsafe {
        remdb::memory::allocator::init_global_allocator(ptr, db_memory.len())
            .unwrap();
    }
    
    remdb::reset_global_db();
    
    // 返回Box，调用者将持有它，确保内存不被释放
    db_memory
}

pub fn setup_test_db() -> Box<[u8]> {
    setup_test_db_with_memory(DEFAULT_TEST_MEMORY_SIZE)
}

pub fn cleanup_test_db() {
    remdb::reset_global_db();
}

#[cfg(feature = "posix")]
pub fn setup_test_db_with_posix_platform(size: usize) -> Box<Vec<u8>> {
    use remdb::platform::posix;
    
    // 使用 static 数组来避免栈溢出
    static mut DB_MEMORY: [u8; 1024 * 1024] = [0u8; 1024 * 1024];
    
    remdb::platform::init_platform(posix::get_posix_platform());
    
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .unwrap();
    }
    
    remdb::reset_global_db();
    
    // 返回一个空的 Box，因为内存已经在 static 数组中
    Box::new(Vec::new())
}

#[cfg(feature = "posix")]
pub fn setup_test_db_with_posix() -> Box<Vec<u8>> {
    setup_test_db_with_posix_platform(DEFAULT_TEST_MEMORY_SIZE)
}
