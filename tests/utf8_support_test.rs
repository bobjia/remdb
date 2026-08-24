extern crate alloc;

use remdb::*;
use serial_test::serial;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 1048576] = [0u8; 1048576]; // 1MB内存

// 定义表结构
remdb::table!(
    users,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(64), // 64字节定长字符串
        email: str(64),
        created_at: u64
    }
);

// 定义数据库配置
remdb::database!(
    TEST_DB,
    tables: [users]
);

#[test]
#[serial]
fn test_utf8_basic_support() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &TEST_DB;

        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 初始化平台抽象层
        #[cfg(feature = "posix")]
        platform::init_platform(platform::posix::get_posix_platform());
        #[cfg(not(feature = "posix"))]
        {
            // 在非posix平台上，使用一个简单的平台实现
            struct DummyPlatform;
            impl platform::Platform for DummyPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {}
                fn spin_unlock(&self, _lock: &mut u32) {}
                fn compiler_barrier(&self) {}
                fn full_memory_barrier(&self) {}
                fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
                    unsafe {
                        core::ptr::copy_nonoverlapping(src, dest, size);
                    }
                }
                fn memset(&self, dest: *mut u8, value: u8, size: usize) {
                    unsafe {
                        core::ptr::write_bytes(dest, value, size);
                    }
                }
                fn delay_ms(&self, _ms: u32) {}
                fn delay_us(&self, _us: u32) {}
                fn file_open(
                    &self,
                    _path: &str,
                    _mode: platform::FileMode,
                ) -> platform::FileResult<platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_write(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *const u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *mut u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(
                    &self,
                    _handle: platform::FileHandle,
                    _offset: i64,
                    _whence: platform::SeekWhence,
                ) -> platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            platform::init_platform(&DUMMY_PLATFORM);
        }

        // 初始化全局数据库
        let db = init_global_db(config).unwrap();

        // 测试1: 插入包含UTF-8字符的记录
        println!("测试1: 插入包含UTF-8字符的记录");

        // 插入中文用户
        let _result = db
            .sql_query(
                "INSERT INTO users VALUES (1, '张三', 'zhangsan@example.com', 1620000000000)",
            )
            .unwrap();
        println!("插入中文用户成功");

        // 插入日文用户
        let _result = db
            .sql_query(
                "INSERT INTO users VALUES (2, '山田太郎', 'yamada@example.com', 1620000001000)",
            )
            .unwrap();
        println!("插入日文用户成功");

        // 插入包含emoji的用户
        let _result = db
            .sql_query(
                "INSERT INTO users VALUES (3, 'Alice 👋', 'alice@example.com', 1620000002000)",
            )
            .unwrap();
        println!("插入包含emoji的用户成功");

        // 测试2: 查询包含UTF-8字符的记录
        println!("\n测试2: 查询包含UTF-8字符的记录");

        // 查询所有用户
        let result = db.sql_query("SELECT * FROM users").unwrap();
        println!("所有用户:\n{}", result.to_string());

        // 根据UTF-8名称查询
        let result = db
            .sql_query("SELECT * FROM users WHERE name = '张三'")
            .unwrap();
        println!("查询中文用户:\n{}", result.to_string());

        // 根据部分UTF-8名称查询
        let result = db
            .sql_query("SELECT * FROM users WHERE name LIKE '%太郎%'")
            .unwrap();
        println!("查询日文用户:\n{}", result.to_string());

        // 测试3: 测试UTF-8字符串函数
        println!("\n测试3: 测试UTF-8字符串函数");

        // 测试LENGTH函数（字节长度）
        let result = db
            .sql_query("SELECT name, LENGTH(name) AS byte_length FROM users")
            .unwrap();
        println!("字符串字节长度:\n{}", result.to_string());

        // 测试CHAR_LENGTH函数（字符长度，UTF-8感知）
        let result = db
            .sql_query("SELECT name, CHAR_LENGTH(name) AS char_length FROM users")
            .unwrap();
        println!("字符串字符长度:\n{}", result.to_string());

        // 测试其他字符串函数
        let result = db
            .sql_query(
                "SELECT name, UPPER(name) AS upper_name, LOWER(name) AS lower_name FROM users",
            )
            .unwrap();
        println!("字符串大小写转换:\n{}", result.to_string());

        // 测试SUBSTRING函数
        let result = db
            .sql_query("SELECT name, SUBSTRING(name, 1, 2) AS substring FROM users")
            .unwrap();
        println!("字符串截取:\n{}", result.to_string());

        // 测试4: 测试UTF-8索引排序
        println!("\n测试4: 测试UTF-8索引排序");

        // 按名称排序（UTF-8感知）
        let result = db.sql_query("SELECT * FROM users ORDER BY name").unwrap();
        println!("按名称排序:\n{}", result.to_string());

        // 测试5: 测试UTF-8字符串比较
        println!("\n测试5: 测试UTF-8字符串比较");

        // 测试大于小于比较
        let result = db
            .sql_query("SELECT * FROM users WHERE name > '李四'")
            .unwrap();
        println!("名称大于'李四'的用户:\n{}", result.to_string());

        // 测试6: 测试UPDATE和DELETE操作
        println!("\n测试6: 测试UPDATE和DELETE操作");

        // 更新UTF-8字符串
        let _result = db
            .sql_query("UPDATE users SET name = '张三更新' WHERE id = 1")
            .unwrap();
        println!("更新中文用户成功");

        // 验证更新结果
        let result = db.sql_query("SELECT * FROM users WHERE id = 1").unwrap();
        println!("更新后结果:\n{}", result.to_string());

        // 删除UTF-8记录
        let _result = db
            .sql_query("DELETE FROM users WHERE name = '山田太郎'")
            .unwrap();
        println!("删除日文用户成功");

        // 验证删除结果
        let result = db.sql_query("SELECT * FROM users").unwrap();
        println!("删除后剩余用户:\n{}", result.to_string());

        println!("\n所有UTF-8支持测试通过！");

        // 清理HA管理器资源
        let _ = ha::shutdown();
    }
}

#[test]
#[serial]
fn test_utf8_validation() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &TEST_DB;

        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 初始化平台抽象层
        #[cfg(not(feature = "posix"))]
        {
            struct DummyPlatform;
            impl platform::Platform for DummyPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {}
                fn spin_unlock(&self, _lock: &mut u32) {}
                fn compiler_barrier(&self) {}
                fn full_memory_barrier(&self) {}
                fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
                    unsafe {
                        core::ptr::copy_nonoverlapping(src, dest, size);
                    }
                }
                fn memset(&self, dest: *mut u8, value: u8, size: usize) {
                    unsafe {
                        core::ptr::write_bytes(dest, value, size);
                    }
                }
                fn delay_ms(&self, _ms: u32) {}
                fn delay_us(&self, _us: u32) {}
                fn file_open(
                    &self,
                    _path: &str,
                    _mode: platform::FileMode,
                ) -> platform::FileResult<platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_write(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *const u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *mut u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(
                    &self,
                    _handle: platform::FileHandle,
                    _offset: i64,
                    _whence: platform::SeekWhence,
                ) -> platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            platform::init_platform(&DUMMY_PLATFORM);
        }

        // 初始化全局数据库
        let _db = init_global_db(config).unwrap();

        println!("测试UTF-8验证");

        // 测试UTF-8处理器的验证功能
        let processor = utf8::Utf8Processor::default();

        // 测试有效UTF-8
        let valid_utf8 = "Hello 世界 👋".as_bytes();
        let result = processor.validate(valid_utf8);
        assert!(matches!(result, utf8::Utf8Result::Ok(_)));
        println!("有效UTF-8验证通过");

        // 测试ASCII
        let ascii = "Hello World".as_bytes();
        let result = processor.validate(ascii);
        assert!(matches!(result, utf8::Utf8Result::Ok(_)));
        println!("ASCII验证通过");

        // 测试字符长度计算
        let char_count = processor.char_length(valid_utf8);
        println!("'Hello 世界 👋' 的字符长度: {}", char_count);
        assert_eq!(char_count, 10); // "Hello " + "世界" + " " + "👋"

        // 测试字符串比较
        let a = "张三".as_bytes();
        let b = "李四".as_bytes();
        let cmp = processor.compare(a, b);
        println!("'张三' 与 '李四' 比较结果: {:?}", cmp);
        assert_eq!(cmp, core::cmp::Ordering::Less);

        println!("所有UTF-8验证测试通过！");

        // 清理HA管理器资源
        let _ = ha::shutdown();
    }
}

#[test]
#[serial]
fn test_utf8_performance() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &TEST_DB;

        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 初始化平台抽象层
        #[cfg(not(feature = "posix"))]
        {
            struct DummyPlatform;
            impl platform::Platform for DummyPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {}
                fn spin_unlock(&self, _lock: &mut u32) {}
                fn compiler_barrier(&self) {}
                fn full_memory_barrier(&self) {}
                fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
                    unsafe {
                        core::ptr::copy_nonoverlapping(src, dest, size);
                    }
                }
                fn memset(&self, dest: *mut u8, value: u8, size: usize) {
                    unsafe {
                        core::ptr::write_bytes(dest, value, size);
                    }
                }
                fn delay_ms(&self, _ms: u32) {}
                fn delay_us(&self, _us: u32) {}
                fn file_open(
                    &self,
                    _path: &str,
                    _mode: platform::FileMode,
                ) -> platform::FileResult<platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_write(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *const u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *mut u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(
                    &self,
                    _handle: platform::FileHandle,
                    _offset: i64,
                    _whence: platform::SeekWhence,
                ) -> platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            platform::init_platform(&DUMMY_PLATFORM);
        }

        // 初始化全局数据库
        let db = init_global_db(config).unwrap();

        println!("测试UTF-8性能");

        // 插入大量UTF-8记录
        let start_time = std::time::Instant::now();

        for i in 4..100 {
            let name = format!("用户{}", i);
            let email = format!("user{}@example.com", i);
            let sql = format!(
                "INSERT INTO users VALUES ({}, '{}', '{}', {})",
                i,
                name,
                email,
                1620000000000 + i as u64 * 1000
            );
            let _ = db.sql_query(&sql).unwrap();
        }

        let insert_duration = start_time.elapsed();
        println!("插入96条UTF-8记录耗时: {:?}", insert_duration);

        // 查询性能测试
        let start_time = std::time::Instant::now();
        let result = db
            .sql_query("SELECT * FROM users WHERE name LIKE '%用户%'")
            .unwrap();
        let query_duration = start_time.elapsed();
        println!(
            "查询UTF-8记录耗时: {:?}, 结果数: {}",
            query_duration,
            result.row_count()
        );

        // 排序性能测试
        let start_time = std::time::Instant::now();
        let _result = db.sql_query("SELECT * FROM users ORDER BY name").unwrap();
        let sort_duration = start_time.elapsed();
        println!("排序UTF-8记录耗时: {:?}", sort_duration);

        println!("所有UTF-8性能测试通过！");

        // 清理HA管理器资源
        let _ = ha::shutdown();
    }
}
