// 导出时序数据相关类型和结构体
pub mod compression;
pub mod index;
pub mod lifecycle;
pub mod partition;
pub mod table;

// 重新导出核心类型
pub use compression::{compress_delta, decompress_delta, CompressionType};
pub use index::TimeSeriesIndex;
pub use lifecycle::{LifecycleManager, RetentionPolicy};
pub use partition::{PartitionManager, PartitionStats, TimeSeriesPartition};
pub use table::{TimeSeriesConfig, TimeSeriesRecord, TimeSeriesTable, TimeSeriesTableDef};
