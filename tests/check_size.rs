// Check struct sizes
#![cfg(all(feature = "std", feature = "ha"))]

use remdb::ha::heartbeat::HeartbeatPacket;
use remdb::ha::HARole;

fn main() {
    println!("Size of HeartbeatPacket: {}", core::mem::size_of::<HeartbeatPacket>());
    println!("Expected size without padding: {}", 8 + 8 + 1 + 4); // u64 + u64 + u8 + u32 = 21 bytes
    
    // Create a packet and print its bytes
    let packet = HeartbeatPacket::new(123, HARole::Master);
    let bytes = packet.to_bytes();
    println!("Packet bytes: {:?}", bytes);
    println!("Byte length: {}", bytes.len());
}
