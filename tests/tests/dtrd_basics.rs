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

use std::{os::unix::process::ExitStatusExt, time::Duration};

use tokio::{
    process::{Child, Command},
    time::sleep,
};

const DUMMY_DATA: &str = "dummydata";

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct DtrdRunner {
    cmd: Child,
}

impl DtrdRunner {
    async fn new() -> Res<Self> {
        let mut path = std::env::current_dir().unwrap();
        path.pop();
        path.push("target");
        path.push("debug");
        path.push("dtrd");
        let cmd = Command::new(path).spawn()?;
        sleep(Duration::from_secs(1)).await;
        Ok(DtrdRunner { cmd })
    }

    async fn stop(mut self) -> Res<()> {
        unsafe {
            libc::kill(self.cmd.id().unwrap() as i32, libc::SIGTERM);
        }
        let exit_code = self.cmd.wait().await?;
        assert_eq!(exit_code.signal().unwrap(), libc::SIGTERM);
        Ok(())
    }
}

impl Drop for DtrdRunner {
    fn drop(&mut self) {
        if let Some(id) = self.cmd.id() {
            unsafe {
                libc::kill(id as i32, libc::SIGKILL);
            }
        }
    }
}

#[tokio::test]
async fn delivers_bundles_locally() -> Result<(), Box<dyn std::error::Error>> {
    let runner = DtrdRunner::new().await?;
    let mut client = dtrd_client::Client::new("http://localhost:50051").await?;
    client
        .submit_bundle(
            "dtn://defaultnodeid/testendpoint",
            60,
            DUMMY_DATA.as_bytes(),
        )
        .await?;
    let data = client
        .receive_bundle("dtn://defaultnodeid/testendpoint")
        .await?;
    assert_eq!(&String::from_utf8(data)?, DUMMY_DATA);
    runner.stop().await?;
    Ok(())
}
