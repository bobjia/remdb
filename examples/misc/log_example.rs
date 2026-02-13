use remdb::log::{debug, error, info, trace, warn, init_logger};

#[cfg(feature = "std")]
fn main() {
    init_logger();

    info!("Starting remdb example");
    debug!("Debug information");
    warn!("Warning message");
    error!("Error occurred");
    trace!("Trace message");

    let data = vec![1, 2, 3, 4, 5];
    info!("Data: {:?}", data);

    info!("Example completed successfully");
}

#[cfg(not(feature = "std"))]
fn main() {
    init_logger();

    info!("Starting remdb no_std example");
    debug!("Debug information");
    warn!("Warning message");
    error!("Error occurred");
    trace!("Trace message");

    let data = [1u8, 2, 3, 4, 5];
    info!("Data: {:?}", data);

    info!("Example completed successfully");
}
