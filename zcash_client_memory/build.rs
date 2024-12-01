use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "src/proto/primitives.proto",
            "src/proto/memory_wallet.proto",
        ],
        &["src/"],
    )?;
    Ok(())
}
