
#![allow(unsafe_code)]

// 导出时序数据相关类型和结构体
pub mod table;
pub mod compression;
pub mod index;
pub mod lifecycle;
pub mod partition;

// 重新导出核心类型
pub use table::{TimeSeriesTable, TimeSeriesTableDef, TimeSeriesRecord, TimeSeriesConfig};
pub use compression::{CompressionType, compress_delta, decompress_delta};
pub use index::{TimeSeriesIndex};
pub use lifecycle::{LifecycleManager, RetentionPolicy};
pub use partition::{TimeSeriesPartition, PartitionManager, PartitionStats};
