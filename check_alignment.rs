use remdb::memory::MemoryBlock;

fn main() {
    println!("Size of MemoryBlock: {}", core::mem::size_of::<MemoryBlock>());
    println!("Alignment of MemoryBlock: {}", core::mem::align_of::<MemoryBlock>());
    println!("Size of Option<NonNull<MemoryBlock>>: {}", core::mem::size_of::<Option<core::ptr::NonNull<MemoryBlock>>>());
    println!("Size of usize: {}", core::mem::size_of::<usize>());
    println!("Size of bool: {}", core::mem::size_of::<bool>());
}