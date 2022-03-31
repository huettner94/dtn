use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path: PathBuf = "../protobuf/bundle/".into();
    let proto_files: Vec<PathBuf> = proto_path
        .read_dir()?
        .filter_map(|p| p.and_then(|path| Ok(path.path())).ok())
        .collect();
    println!("{:?}", proto_files);
    tonic_build::configure()
        .build_client(false)
        .compile(&proto_files, &[proto_path])?;
    Ok(())
}
