//! 自定义同步原语
//!
//! 提供不会中毒的 Mutex 和 RwLock 实现，消除 `lock().unwrap()` 的需要。
//! 在 std 环境下包装 std::sync::Mutex/RwLock 并处理中毒情况，
//! 在 no_std 环境下使用自旋锁实现。

// ============================
// Mutex
// ============================

/// 不会中毒的 Mutex
///
/// 在 std 环境下，如果锁中毒，会自动恢复（继续使用数据）。
/// 在 no_std 环境下，使用自旋锁实现。
pub struct Mutex<T> {
    #[cfg(feature = "std")]
    inner: std::sync::Mutex<T>,
    #[cfg(not(feature = "std"))]
    inner: crate::memory::allocator::Mutex<T>,
}

impl<T> Mutex<T> {
    /// 创建新的 Mutex
    pub fn new(value: T) -> Self {
        Mutex {
            #[cfg(feature = "std")]
            inner: std::sync::Mutex::new(value),
            #[cfg(not(feature = "std"))]
            inner: crate::memory::allocator::Mutex::new(value),
        }
    }

    /// 获取锁，返回守卫
    /// 如果锁中毒，会自动恢复，不会 panic
    pub fn lock(&self) -> MutexGuard<'_, T> {
        #[cfg(feature = "std")]
        {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            MutexGuard { inner: guard }
        }
        #[cfg(not(feature = "std"))]
        {
            // no_std 版本的 Mutex 永远不会返回错误
            let guard = try_lock!(self.inner);
            MutexGuard { inner: guard }
        }
    }

    /// 尝试获取锁，如果无法立即获取则返回 None
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        #[cfg(feature = "std")]
        {
            match self.inner.try_lock() {
                Ok(guard) => Some(MutexGuard { inner: guard }),
                Err(_) => None,
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner
                .try_lock()
                .ok()
                .map(|guard| MutexGuard { inner: guard })
        }
    }
}

/// Mutex 守卫
pub struct MutexGuard<'a, T> {
    #[cfg(feature = "std")]
    inner: std::sync::MutexGuard<'a, T>,
    #[cfg(not(feature = "std"))]
    inner: crate::memory::allocator::MutexGuard<'a, T>,
}

impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}

impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.inner
    }
}

// ============================
// RwLock
// ============================

/// 不会中毒的读写锁
pub struct RwLock<T> {
    #[cfg(feature = "std")]
    inner: std::sync::RwLock<T>,
    #[cfg(not(feature = "std"))]
    inner: crate::memory::allocator::Mutex<T>, // no_std 下使用 Mutex 模拟 RwLock
}

impl<T> RwLock<T> {
    /// 创建新的 RwLock
    pub fn new(value: T) -> Self {
        RwLock {
            #[cfg(feature = "std")]
            inner: std::sync::RwLock::new(value),
            #[cfg(not(feature = "std"))]
            inner: crate::memory::allocator::Mutex::new(value),
        }
    }

    /// 获取读锁
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        #[cfg(feature = "std")]
        {
            let guard = self.inner.read().unwrap_or_else(|e| e.into_inner());
            RwLockReadGuard { inner: guard }
        }
        #[cfg(not(feature = "std"))]
        {
            // no_std 下使用 Mutex 模拟，但只提供读访问
            // 注意：这里没有真正的读写锁语义，只是简单的互斥锁
            // 对于 no_std 场景，性能不是关键，正确性更重要
            let guard = try_lock!(self.inner);
            RwLockReadGuard { inner: guard }
        }
    }

    /// 尝试获取读锁
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        #[cfg(feature = "std")]
        {
            match self.inner.try_read() {
                Ok(guard) => Some(RwLockReadGuard { inner: guard }),
                Err(_) => None,
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner
                .try_lock()
                .ok()
                .map(|guard| RwLockReadGuard { inner: guard })
        }
    }

    /// 获取写锁
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        #[cfg(feature = "std")]
        {
            let guard = self.inner.write().unwrap_or_else(|e| e.into_inner());
            RwLockWriteGuard { inner: guard }
        }
        #[cfg(not(feature = "std"))]
        {
            let guard = try_lock!(self.inner);
            RwLockWriteGuard { inner: guard }
        }
    }

    /// 尝试获取写锁
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        #[cfg(feature = "std")]
        {
            match self.inner.try_write() {
                Ok(guard) => Some(RwLockWriteGuard { inner: guard }),
                Err(_) => None,
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner
                .try_lock()
                .ok()
                .map(|guard| RwLockWriteGuard { inner: guard })
        }
    }
}

/// 读锁守卫
pub struct RwLockReadGuard<'a, T> {
    #[cfg(feature = "std")]
    inner: std::sync::RwLockReadGuard<'a, T>,
    #[cfg(not(feature = "std"))]
    inner: crate::memory::allocator::MutexGuard<'a, T>,
}

impl<'a, T> core::ops::Deref for RwLockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}

/// 写锁守卫
pub struct RwLockWriteGuard<'a, T> {
    #[cfg(feature = "std")]
    inner: std::sync::RwLockWriteGuard<'a, T>,
    #[cfg(not(feature = "std"))]
    inner: crate::memory::allocator::MutexGuard<'a, T>,
}

impl<'a, T> core::ops::Deref for RwLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}

impl<'a, T> core::ops::DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.inner
    }
}

// ============================
// Arc
// ============================

/// 线程安全的引用计数指针
#[cfg(feature = "std")]
pub use std::sync::Arc;

#[cfg(not(feature = "std"))]
pub use alloc::sync::Arc;
