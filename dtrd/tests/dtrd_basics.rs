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

use std::sync::atomic::AtomicU16;
use std::{process::Stdio, time::Duration};

use tokio::{
    process::{Child, Command},
    time::sleep,
};

const DUMMY_DATA: &str = "dummydata";
const DTRD_BIN_PATH: &str = env!("CARGO_BIN_EXE_dtrd");

static PORT_COUNTER: AtomicU16 = AtomicU16::new(50000);

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct DtrdRunner {
    cmd: Option<Child>,
}

impl DtrdRunner {
    async fn new(node_id: &str, grpc_port: u16, tcpcl_port: u16) -> Res<Self> {
        let cmd = Command::new(DTRD_BIN_PATH)
            .env("NODE_ID", node_id)
            .env(
                "GRPC_CLIENTAPI_ADDRESS",
                &format!("127.0.0.1:{}", grpc_port),
            )
            .env("TCPCL_LISTEN_ADDRESS", &format!("127.0.0.1:{}", tcpcl_port))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        sleep(Duration::from_secs(1)).await;
        Ok(DtrdRunner { cmd: Some(cmd) })
    }

    async fn stop(mut self) -> Res<()> {
        unsafe {
            libc::kill(
                self.cmd.as_ref().unwrap().id().unwrap() as i32,
                libc::SIGINT,
            );
        }
        let output = self.cmd.take().unwrap().wait_with_output().await?;
        assert_eq!(output.status.code().unwrap(), 0, "Did not exit with 0");

        // We log to stderr per default
        let stderr = String::from_utf8(output.stderr).unwrap();
        let mut lines = stderr.lines();

        while let Some(line) = lines.next() {
            println!("{}", line);
            assert!(
                line.split(']').next().unwrap().contains(" INFO "),
                "Had log line that was not INFO"
            );
        }

        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(stdout.len(), 0, "Stdout should be empty");

        Ok(())
    }
}

impl Drop for DtrdRunner {
    fn drop(&mut self) {
        if let Some(id) = self.cmd.as_ref().and_then(|c| c.id()) {
            unsafe {
                libc::kill(id as i32, libc::SIGKILL);
            }
        }
    }
}

struct Dtrd {
    runner: DtrdRunner,
    client: dtrd_client::Client,
    grpc_port: u16,
    tcpcl_port: u16,
    node_id: String,
}

impl Dtrd {
    async fn new() -> Res<Self> {
        let port_range = PORT_COUNTER.fetch_add(10, std::sync::atomic::Ordering::SeqCst);
        let node_id = format!("dtn://testrunnode{}", port_range);
        let grpc_port = port_range + 1;
        let tcpcl_port = port_range + 2;
        let runner = DtrdRunner::new(&node_id, grpc_port, tcpcl_port).await?;
        let client = dtrd_client::Client::new(&format!("http://127.0.0.1:{}", grpc_port)).await?;
        Ok(Dtrd {
            runner,
            client,
            grpc_port,
            tcpcl_port,
            node_id,
        })
    }

    async fn stop(self) -> Res<()> {
        self.runner.stop().await
    }

    fn with_node_id(&self, suffix: &str) -> String {
        format!("{}/{}", self.node_id, suffix)
    }

    async fn connect_to(&mut self, other: &Dtrd) -> Res<()> {
        self.client
            .add_node(format!("tcpcl://127.0.0.1:{}", other.tcpcl_port))
            .await?;
        sleep(Duration::from_secs(1)).await;
        assert!(
            self.client
                .list_nodes()
                .await?
                .iter()
                .any(|e| e.endpoint == other.node_id)
        );
        Ok(())
    }
}

#[tokio::test]
async fn delivers_bundles_locally() -> Result<(), Box<dyn std::error::Error>> {
    let mut dtrd = Dtrd::new().await?;
    dtrd.client
        .submit_bundle(
            &dtrd.with_node_id("testendpoint"),
            60,
            DUMMY_DATA.as_bytes(),
            false,
        )
        .await?;
    let data = dtrd
        .client
        .receive_bundle(&dtrd.with_node_id("testendpoint"))
        .await?;
    assert_eq!(&String::from_utf8(data)?, DUMMY_DATA);
    dtrd.stop().await?;
    Ok(())
}

#[tokio::test]
async fn delivers_bundles_connected() -> Result<(), Box<dyn std::error::Error>> {
    let mut dtrd1 = Dtrd::new().await?;
    let mut dtrd2 = Dtrd::new().await?;
    dtrd1.connect_to(&dtrd2).await?;
    dtrd1
        .client
        .submit_bundle(
            &dtrd2.with_node_id("testendpoint"),
            60,
            DUMMY_DATA.as_bytes(),
            false,
        )
        .await?;
    let data = dtrd2
        .client
        .receive_bundle(&dtrd2.with_node_id("testendpoint"))
        .await?;
    assert_eq!(&String::from_utf8(data)?, DUMMY_DATA);
    dtrd1.stop().await?;
    dtrd2.stop().await?;
    Ok(())
}

#[tokio::test]
async fn delivers_bundles_routed() -> Result<(), Box<dyn std::error::Error>> {
    let mut dtrd1 = Dtrd::new().await?;
    let mut dtrd2 = Dtrd::new().await?;
    let mut dtrd3 = Dtrd::new().await?;
    dtrd1.connect_to(&dtrd2).await?;
    dtrd2.connect_to(&dtrd3).await?;
    dtrd1
        .client
        .add_route(dtrd3.node_id.clone(), dtrd2.node_id.clone())
        .await?;
    dtrd1
        .client
        .submit_bundle(
            &dtrd3.with_node_id("testendpoint"),
            60,
            DUMMY_DATA.as_bytes(),
            false,
        )
        .await?;
    let data = dtrd3
        .client
        .receive_bundle(&dtrd3.with_node_id("testendpoint"))
        .await?;
    assert_eq!(&String::from_utf8(data)?, DUMMY_DATA);
    dtrd1.stop().await?;
    dtrd2.stop().await?;
    dtrd3.stop().await?;
    Ok(())
}
