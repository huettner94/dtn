fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../protobuf/bundle/bundle.proto")?;
    Ok(())
}
