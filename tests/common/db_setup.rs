//! 数据库初始化辅助函数
//!
//! 提供统一的数据库初始化和清理函数

use std::sync::Mutex;

use super::platform::TEST_PLATFORM;

const DEFAULT_TEST_MEMORY_SIZE: usize = 1024 * 1024; // 1MB

/// 保存当前测试的数据库内存池，确保内存池在测试期间保持有效
/// 并且在下一次测试调用 setup_test_db 时才释放旧的内存池
static TEST_DB_MEMORY: Mutex<Option<Box<[u8]>>> = Mutex::new(None);

pub fn setup_test_db_with_memory(size: usize) {
    // 1. 取出旧的内存池（如果存在），保持其在局部变量中存活
    let old_pool = TEST_DB_MEMORY.lock().unwrap().take();

    // 2. 先释放旧数据库。旧的内存池仍然在 old_pool 中存活，
    //    所以旧的分配器仍然有效，MemoryTable::drop() 中的 free() 调用能正确工作
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::reset_global_db();

    // 3. 现在可以安全地释放旧的内存池
    drop(old_pool);

    // 4. 分配新的内存池
    let mut db_memory = vec![0u8; size].into_boxed_slice();
    let ptr = db_memory.as_mut_ptr();

    // 5. 初始化新的全局分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(ptr, db_memory.len()).unwrap();
    }

    // 6. 将新的内存池保存到静态变量中，确保其在整个测试期间有效
    *TEST_DB_MEMORY.lock().unwrap() = Some(db_memory);
}

pub fn setup_test_db() {
    setup_test_db_with_memory(DEFAULT_TEST_MEMORY_SIZE)
}

pub fn cleanup_test_db() {
    remdb::reset_global_db();
}

#[cfg(feature = "posix")]
pub fn setup_test_db_with_posix_platform(_size: usize) -> Box<Vec<u8>> {
    use remdb::platform::posix;

    // 使用 static 数组来避免栈溢出
    static mut DB_MEMORY: [u8; 1024 * 1024] = [0u8; 1024 * 1024];

    remdb::platform::init_platform(posix::get_posix_platform());

    // 先释放旧数据库，此时旧分配器仍然有效（static数组始终有效）
    remdb::reset_global_db();

    // 再初始化新的分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .unwrap();
    }

    // 返回一个空的 Box，因为内存已经在 static 数组中
    Box::new(Vec::new())
}

#[cfg(feature = "posix")]
pub fn setup_test_db_with_posix() -> Box<Vec<u8>> {
    setup_test_db_with_posix_platform(DEFAULT_TEST_MEMORY_SIZE)
}
