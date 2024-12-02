use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "src/proto/memory_wallet.proto",
            "src/proto/notes.proto",
            "src/proto/primitives.proto",
            "src/proto/shardtree.proto",
            "src/proto/transparent.proto",
        ],
        &["src/"],
    )?;
    Ok(())
}
