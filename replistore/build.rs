// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::path::PathBuf;

fn build_tonic() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path: PathBuf = "../protobuf/replistore/".into();
    let proto_files: Vec<PathBuf> = proto_path
        .read_dir()?
        .filter_map(|p| p.map(|path| path.path()).ok())
        .collect();
    println!("{proto_files:?}");
    tonic_prost_build::configure()
        .build_client(false)
        .compile_protos(&proto_files, &[proto_path])?;
    Ok(())
}

fn build_prost() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path: PathBuf = "./protobuf/".into();
    let proto_files: Vec<PathBuf> = proto_path
        .read_dir()?
        .filter_map(|p| p.map(|path| path.path()).ok())
        .collect();
    println!("{proto_files:?}");
    prost_build::compile_protos(&proto_files, &[proto_path])?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    build_tonic()?;
    build_prost()
}
