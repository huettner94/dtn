use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path: PathBuf = "../protobuf/bundle/".into();
    let proto_files: Vec<PathBuf> = proto_path
        .read_dir()?
        .filter_map(|p| p.map(|path| path.path()).ok())
        .collect();
    println!("{:?}", proto_files);
    tonic_build::configure()
        .build_server(false)
        .compile(&proto_files, &[proto_path])?;
    Ok(())
}
