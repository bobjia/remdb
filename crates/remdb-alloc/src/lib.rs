//! remdb-alloc: Memory allocator for remdb
//!
//! This crate provides a static memory allocator that works in `no_std` environments.
//! It manages a fixed-size memory pool using a free-list algorithm with coalescing.
//!
//! # Safety
//!
//! This crate contains inherently unsafe memory allocation code:
//! - `unsafe impl GlobalAlloc` for `no_std` global allocator support
//! - `UnsafeCell`-based `Mutex` for `no_std` synchronization
//! - Raw pointer manipulation for `MemoryBlock` linked list management
//! - `NonNull` pointer arithmetic for block header traversal
//!
//! All unsafe code is confined to this crate and documented with safety comments.
//! The public API is safe to use.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unsafe_code)]

use core::ptr::NonNull;

// ============================================================================
// Conditional std imports
// ============================================================================

#[cfg(feature = "std")]
use std::sync::OnceLock;

#[cfg(feature = "std")]
use std::sync::Mutex;

// ============================================================================
// no_std synchronization primitives
// ============================================================================

/// A thread-safe one-time initializer for `no_std` environments.
///
/// Provides the same API as `std::sync::OnceLock` but without requiring `std`.
#[cfg(not(feature = "std"))]
pub struct OnceLock<T> {
    data: core::cell::UnsafeCell<Option<T>>,
    initialized: core::sync::atomic::AtomicBool,
}

#[cfg(not(feature = "std"))]
unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}

#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Send for OnceLock<T> {}

#[cfg(not(feature = "std"))]
impl<T> OnceLock<T> {
    /// Create a new, empty `OnceLock`.
    pub const fn new() -> Self {
        OnceLock {
            data: core::cell::UnsafeCell::new(None),
            initialized: core::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Get a reference to the contained value, if any.
    pub fn get(&self) -> Option<&T> {
        if self.initialized.load(core::sync::atomic::Ordering::Acquire) {
            unsafe { (*self.data.get()).as_ref() }
        } else {
            None
        }
    }

    /// Set the contained value. Returns `Err(value)` if already initialized.
    pub fn set(&self, value: T) -> core::result::Result<(), T> {
        if self.initialized.swap(true, core::sync::atomic::Ordering::AcqRel) {
            Err(value)
        } else {
            unsafe {
                *self.data.get() = Some(value);
            }
            Ok(())
        }
    }

    /// Get or initialize the contained value.
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        match self.get() {
            Some(v) => v,
            None => {
                let _ = self.set(f());
                unsafe { (*self.data.get()).as_ref().unwrap() }
            }
        }
    }
}

/// A simple spinlock-based mutex for `no_std` environments.
#[cfg(not(feature = "std"))]
pub struct Mutex<T> {
    data: core::cell::UnsafeCell<T>,
    lock: u32,
}

#[cfg(not(feature = "std"))]
impl<T> Mutex<T> {
    /// Create a new `Mutex` wrapping the given value.
    pub fn new(data: T) -> Self {
        Mutex {
            data: core::cell::UnsafeCell::new(data),
            lock: 0,
        }
    }

    /// Lock the mutex, blocking until acquired.
    pub fn lock(&self) -> core::result::Result<MutexGuard<'_, T>, ()> {
        // Simple spinlock: atomically exchange 0→1
        while unsafe {
            core::sync::atomic::AtomicU32::from_ptr(&self.lock as *const u32 as *mut u32)
                .compare_exchange(
                    0,
                    1,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_err()
        } {
            core::hint::spin_loop();
        }

        Ok(MutexGuard { mutex: self })
    }
}

#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Sync for Mutex<T> {}

#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Send for Mutex<T> {}

/// A guard that releases the mutex when dropped.
#[cfg(not(feature = "std"))]
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

#[cfg(not(feature = "std"))]
impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            core::sync::atomic::AtomicU32::from_ptr(&self.mutex.lock as *const u32 as *mut u32)
                .store(0, core::sync::atomic::Ordering::Release);
        }
    }
}

// ============================================================================
// Result type alias
// ============================================================================

/// Convenience alias for allocator operations returning `&'static str` errors.
pub type AllocResult<T> = core::result::Result<T, &'static str>;

// ============================================================================
// MemoryBlock and MemoryStats
// ============================================================================

/// Memory block header for the free-list allocator.
///
/// Each allocated or free block in the memory pool begins with this header.
/// Free blocks form a singly-linked list via `next`.
#[repr(C)]
pub struct MemoryBlock {
    /// Next block in the free list, or `None` if this is the last block.
    pub next: Option<NonNull<MemoryBlock>>,
    /// Size of the data portion of this block (excluding the header).
    pub size: usize,
    /// Whether this block is currently allocated.
    pub is_allocated: bool,
}

impl MemoryBlock {
    /// Size of the `MemoryBlock` header itself.
    pub const SIZE: usize = core::mem::size_of::<Self>();
}

/// Memory allocation statistics.
pub struct MemoryStats {
    /// Number of bytes currently in use.
    pub used: usize,
    /// Total size of the managed memory pool (bytes).
    pub total: usize,
    /// Fragmentation ratio: `0.0` means no fragmentation, `1.0` means maximum.
    pub fragmentation: f32,
    /// Number of successful allocation calls.
    pub alloc_count: usize,
    /// Number of free calls.
    pub free_count: usize,
}

// ============================================================================
// MemoryAllocator trait
// ============================================================================

/// Pluggable memory allocator trait.
///
/// Implementations manage a memory pool and provide allocation/deallocation.
pub trait MemoryAllocator: Sync {
    /// Allocate a block of memory of at least `size` bytes.
    fn allocate(&self, size: usize) -> Option<NonNull<u8>>;

    /// Deallocate a previously allocated block.
    ///
    /// # Safety
    ///
    /// `ptr` must have been returned by a previous call to `allocate` on the
    /// same allocator, and must not have been deallocated yet.
    fn deallocate(&self, ptr: NonNull<u8>, size: usize);
}

// ============================================================================
// StaticAllocator — free-list allocator
// ============================================================================

/// Static memory allocator using a free-list algorithm with coalescing.
///
/// Manages a fixed-size memory region. Allocation uses a first-fit strategy.
/// Adjacent free blocks are automatically merged on deallocation.
pub struct StaticAllocator {
    /// Aligned start of the managed memory pool.
    start_ptr: NonNull<u8>,
    /// Total size of the managed memory pool (after alignment).
    size: usize,
    /// Number of bytes currently in use (includes block headers).
    used: usize,
    /// Head of the free-list.
    free_list: Option<NonNull<MemoryBlock>>,
    /// Number of allocation calls made.
    alloc_count: usize,
    /// Number of free calls made.
    free_count: usize,
}

// SAFETY: The `NonNull` fields are only accessed behind a mutex or during
//         single-threaded construction; the allocator is `Send` and `Sync`.
unsafe impl Send for StaticAllocator {}
unsafe impl Sync for StaticAllocator {}

impl Clone for StaticAllocator {
    fn clone(&self) -> Self {
        Self::new(self.start_ptr.as_ptr() as *mut u8, self.size)
            .expect("Failed to clone StaticAllocator")
    }
}

impl StaticAllocator {
    /// Required alignment for `MemoryBlock` headers.
    const ALIGNMENT: usize = core::mem::align_of::<MemoryBlock>();

    /// Create a new `StaticAllocator` managing the given memory region.
    ///
    /// The `start_ptr` is aligned up to `MemoryBlock` alignment. Returns `None`
    /// if the region is too small to hold even one block header.
    pub fn new(start_ptr: *mut u8, size: usize) -> Option<Self> {
        let start_addr = start_ptr as usize;
        let aligned_addr = (start_addr + Self::ALIGNMENT - 1) & !(Self::ALIGNMENT - 1);
        let aligned_ptr = aligned_addr as *mut u8;
        let aligned_size = size - (aligned_addr - start_addr);

        if aligned_size < MemoryBlock::SIZE {
            return None;
        }

        let mut allocator = StaticAllocator {
            start_ptr: NonNull::new(aligned_ptr)?,
            size: aligned_size,
            used: 0,
            free_list: None,
            alloc_count: 0,
            free_count: 0,
        };

        allocator.reset();
        Some(allocator)
    }

    /// Reset the allocator, reinitialising the entire pool as a single free block.
    pub fn reset(&mut self) {
        // SAFETY: `start_ptr` is guaranteed to be valid and aligned, and
        //         `size` is large enough for at least one `MemoryBlock`.
        unsafe {
            let block_ptr = self.start_ptr.as_ptr() as *mut MemoryBlock;
            (*block_ptr).next = None;
            (*block_ptr).size = self.size - MemoryBlock::SIZE;
            (*block_ptr).is_allocated = false;

            self.free_list = Some(NonNull::new_unchecked(block_ptr));
            self.used = 0;
            self.alloc_count = 0;
            self.free_count = 0;
        }
    }

    /// Allocate a block of memory of at least `size` bytes.
    ///
    /// Uses a first-fit search through the free list. Returns a pointer to the
    /// data portion of the block (after the header).
    pub fn allocate(&mut self, size: usize) -> AllocResult<NonNull<u8>> {
        // Align requested size to 8 bytes.
        let aligned_size = (size + 7) & !7;
        let total_size = aligned_size + MemoryBlock::SIZE;

        let mut current = &mut self.free_list;
        while let Some(mut block) = *current {
            // SAFETY: `block` is a valid, non-null pointer into the managed pool.
            let block_mut = unsafe { block.as_mut() };

            if block_mut.size >= aligned_size {
                // Split the block if it is large enough to leave a usable remainder.
                if block_mut.size >= aligned_size + MemoryBlock::SIZE + 8 {
                    // SAFETY: The new block fits entirely within the managed pool.
                    unsafe {
                        let new_block_size = block_mut.size - total_size;
                        let new_block_ptr =
                            (block.as_ptr() as usize + total_size) as *mut MemoryBlock;

                        (*new_block_ptr).next = block_mut.next;
                        (*new_block_ptr).size = new_block_size;
                        (*new_block_ptr).is_allocated = false;

                        block_mut.next = Some(NonNull::new_unchecked(new_block_ptr));
                        block_mut.size = aligned_size;
                    }
                }

                // Remove this block from the free list.
                let _allocated_block = *current;
                *current = unsafe { block.as_mut() }.next;

                // SAFETY: The block is now exclusively owned by the caller.
                unsafe {
                    block.as_mut().is_allocated = true;
                }

                self.used += unsafe { block.as_mut() }.size + MemoryBlock::SIZE;
                self.alloc_count += 1;

                // Return a pointer to the data portion (past the header).
                let data_ptr = (block.as_ptr() as usize + MemoryBlock::SIZE) as *mut u8;
                // SAFETY: `data_ptr` is derived from a non-null block pointer
                //         offset by a positive amount, so it is never null.
                return Ok(unsafe { NonNull::new_unchecked(data_ptr) });
            }

            current = &mut unsafe { block.as_mut() }.next;
        }

        Err("Out of memory")
    }

    /// Free a previously allocated block.
    ///
    /// The block is re-inserted into the free list in address order, and
    /// adjacent free blocks are merged.
    pub fn free(&mut self, ptr: NonNull<u8>) {
        // SAFETY: `ptr` was returned by a previous `allocate` call, so the
        //         block header is at `ptr - MemoryBlock::SIZE` and is valid.
        let block_ptr = (ptr.as_ptr() as usize - MemoryBlock::SIZE) as *mut MemoryBlock;
        // SAFETY: `block_ptr` is derived from a valid allocation, so it is
        //         non-null and properly aligned.
        let mut block = unsafe { NonNull::new_unchecked(block_ptr) };

        // Mark as free.
        unsafe {
            block.as_mut().is_allocated = false;
        }

        // Update statistics.
        let block_size = unsafe { block.as_mut() }.size + MemoryBlock::SIZE;
        if self.used >= block_size {
            self.used -= block_size;
        }
        self.free_count += 1;

        // Insert into the free list, keeping it sorted by address.
        let mut current = &mut self.free_list;
        while let Some(mut current_block) = *current {
            if current_block.as_ptr() > block.as_ptr() {
                // Insert before `current_block`.
                unsafe {
                    block.as_mut().next = Some(current_block);
                }
                *current = Some(block);
                self.merge_adjacent_blocks();
                return;
            }
            current = &mut unsafe { current_block.as_mut() }.next;
        }

        // Append to the end of the list.
        unsafe {
            block.as_mut().next = None;
        }
        *current = Some(block);
        self.merge_adjacent_blocks();
    }

    /// Merge adjacent free blocks to reduce fragmentation.
    fn merge_adjacent_blocks(&mut self) {
        let mut current = &mut self.free_list;
        while let Some(mut block) = *current {
            // SAFETY: `block` is a valid pointer in the free list.
            let block_mut = unsafe { block.as_mut() };

            if let Some(mut next_block) = block_mut.next {
                // SAFETY: `next_block` is a valid pointer in the free list.
                let next_block_mut = unsafe { next_block.as_mut() };
                let block_end = block.as_ptr() as usize + MemoryBlock::SIZE + block_mut.size;
                let next_block_start = next_block.as_ptr() as usize;

                if block_end == next_block_start {
                    // Absorb the next block.
                    block_mut.size += MemoryBlock::SIZE + next_block_mut.size;
                    block_mut.next = next_block_mut.next;
                    // Restart the loop to check for further merges.
                    continue;
                }
            }

            current = &mut block_mut.next;
        }
    }

    /// Return memory statistics for this allocator instance.
    pub fn stats(&self) -> MemoryStats {
        let mut free_blocks = 0;
        let mut max_free_block = 0;
        let mut total_free = 0;

        let mut current = self.free_list;
        while let Some(block) = current {
            free_blocks += 1;
            // SAFETY: `block` is a valid pointer in the free list.
            unsafe {
                total_free += block.as_ref().size + MemoryBlock::SIZE;
                if block.as_ref().size > max_free_block {
                    max_free_block = block.as_ref().size;
                }
                current = block.as_ref().next;
            }
        }

        let fragmentation = if free_blocks == 0 {
            0.0
        } else {
            1.0 - (max_free_block as f32 / total_free as f32)
        };

        MemoryStats {
            used: self.used,
            total: self.size,
            fragmentation,
            alloc_count: self.alloc_count,
            free_count: self.free_count,
        }
    }
}

// ============================================================================
// Global allocator state
// ============================================================================

/// The global allocator instance, lazily initialised via `init_global_allocator`.
static GLOBAL_ALLOCATOR: OnceLock<Mutex<StaticAllocator>> = OnceLock::new();

/// Initialise the global memory allocator with the given memory region.
///
/// If the allocator was already initialised, the existing instance is replaced
/// with a fresh one (all prior allocations are invalidated).
pub fn init_global_allocator(start_ptr: *mut u8, size: usize) -> AllocResult<()> {
    let new_allocator = StaticAllocator::new(start_ptr, size)
        .ok_or("Out of memory: unable to create StaticAllocator")?;

    if let Some(allocator_mutex) = GLOBAL_ALLOCATOR.get() {
        let mut allocator_guard = allocator_mutex
            .lock()
            .map_err(|_| "Failed to lock allocator")?;
        *allocator_guard = new_allocator;
    } else {
        let _ = GLOBAL_ALLOCATOR.set(Mutex::new(new_allocator));
    }

    Ok(())
}

/// Allocate a block of memory from the global allocator.
pub fn alloc(size: usize) -> AllocResult<NonNull<u8>> {
    let allocator = GLOBAL_ALLOCATOR
        .get()
        .ok_or("Allocator not initialised")?;
    let mut allocator_guard = allocator
        .lock()
        .map_err(|_| "Failed to lock allocator")?;
    allocator_guard.allocate(size)
}

/// Free a block of memory back to the global allocator.
pub fn free(ptr: NonNull<u8>) -> AllocResult<()> {
    let allocator = GLOBAL_ALLOCATOR
        .get()
        .ok_or("Allocator not initialised")?;
    let mut allocator_guard = allocator
        .lock()
        .map_err(|_| "Failed to lock allocator")?;
    allocator_guard.free(ptr);
    Ok(())
}

/// Return memory statistics from the global allocator.
pub fn get_memory_stats() -> MemoryStats {
    if let Some(allocator) = GLOBAL_ALLOCATOR.get() {
        if let Ok(allocator_guard) = allocator.lock() {
            return allocator_guard.stats();
        }
    }

    MemoryStats {
        used: 0,
        total: 0,
        fragmentation: 0.0,
        alloc_count: 0,
        free_count: 0,
    }
}

/// Reset the global allocator, reinitialising its memory pool.
pub fn reset_allocator() -> AllocResult<()> {
    if let Some(allocator) = GLOBAL_ALLOCATOR.get() {
        let mut allocator_guard = allocator
            .lock()
            .map_err(|_| "Failed to lock allocator")?;
        allocator_guard.reset();
    }
    Ok(())
}

// ============================================================================
// GlobalAllocator — `core::alloc::GlobalAlloc` impl for `no_std`
// ============================================================================

/// A `no_std` global allocator that delegates to the allocator in this crate.
///
/// This is the `#[global_allocator]` for `no_std` targets. When the `std`
/// feature is enabled, the standard library allocator is used instead.
#[cfg(not(feature = "std"))]
pub struct GlobalAllocator;

#[cfg(not(feature = "std"))]
unsafe impl core::alloc::GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        match alloc(layout.size()) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        if let Some(non_null_ptr) = core::ptr::NonNull::new(ptr) {
            let _ = free(non_null_ptr);
        }
    }
}

/// The `no_std` global allocator instance.
#[cfg(not(feature = "std"))]
#[global_allocator]
pub static GLOBAL_ALLOC: GlobalAllocator = GlobalAllocator;