fn main() {
    let mut config = prost_build::Config::new();
    config.compile_protos(&["proto/remdb.proto"], &["proto/"]).unwrap_or_else(|e| {
        panic!("failed to compile protos: {}", e)
    });
    println!("cargo:rerun-if-changed=proto/remdb.proto");
}