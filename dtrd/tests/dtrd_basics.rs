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

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU16;
use std::{process::Stdio, time::Duration};

use bp7::administrative_record::AdministrativeRecord;
use bp7::administrative_record::bundle_status_report::BundleStatusReason;
use tokio::fs;
use tokio::{
    process::{Child, Command},
    time::sleep,
};
use tokio_util::time::FutureExt;

const DUMMY_DATA: &str = "dummydata";
const DTRD_BIN_PATH: &str = env!("CARGO_BIN_EXE_dtrd");

static PORT_COUNTER: AtomicU16 = AtomicU16::new(50000);

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct DtrdRunner {
    cmd: Option<Child>,
}

impl DtrdRunner {
    async fn new(node_id: &str, grpc_port: u16, tcpcl_port: u16, bundle_dir: &Path) -> Res<Self> {
        let mut runner = DtrdRunner { cmd: None };
        runner
            .start(node_id, grpc_port, tcpcl_port, bundle_dir)
            .await?;
        Ok(runner)
    }

    async fn start(
        &mut self,
        node_id: &str,
        grpc_port: u16,
        tcpcl_port: u16,
        bundle_dir: &Path,
    ) -> Res<()> {
        assert!(self.cmd.is_none(), "need to stop first");
        let cmd = Command::new(DTRD_BIN_PATH)
            .env("NODE_ID", node_id)
            .env("GRPC_CLIENTAPI_ADDRESS", format!("127.0.0.1:{grpc_port}"))
            .env("TCPCL_LISTEN_ADDRESS", format!("127.0.0.1:{tcpcl_port}"))
            .env(
                "BUNDLE_STORAGE_PATH",
                bundle_dir.to_string_lossy().to_string(),
            )
            .env("RUST_LOG", "info,dtrd=debug")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        sleep(Duration::from_secs(1)).await;
        self.cmd = Some(cmd);
        Ok(())
    }

    async fn stop(&mut self, allowed_messages: &[String]) -> Res<()> {
        let mut out = Ok(());
        if self.cmd.is_none() {
            // We were stopping anyway so we are either fine or have already an error
            return Ok(());
        }
        unsafe {
            #[allow(clippy::cast_possible_wrap)]
            libc::kill(
                self.cmd.as_ref().unwrap().id().unwrap() as i32,
                libc::SIGINT,
            );
        }
        let output = self.cmd.take().unwrap().wait_with_output().await?;
        if output.status.code().unwrap() != 0 {
            out = Err("Did not exit with 0".into());
        }

        // We log to stderr per default
        let stderr = String::from_utf8(output.stderr).unwrap();
        let lines = stderr.lines();

        for line in lines {
            println!("{line}");
            // Explicit allowlisted error.
            if allowed_messages.iter().any(|e| line.contains(e)) {
                continue;
            }
            let header = line.split(']').next().unwrap();
            if !(header.contains(" INFO ") || header.contains(" DEBUG ")) && out.is_ok() {
                out = Err("Had log line that was not INFO or DEBUG".into());
            }
        }

        let stdout = String::from_utf8(output.stdout).unwrap();
        if !stdout.is_empty() && out.is_ok() {
            out = Err("Stdout was not empty".into());
        }
        let lines = stdout.lines();
        for line in lines {
            println!("STDOUT: {line}");
        }

        out
    }
}

impl Drop for DtrdRunner {
    fn drop(&mut self) {
        if let Some(id) = self.cmd.as_ref().and_then(tokio::process::Child::id) {
            unsafe {
                #[allow(clippy::cast_possible_wrap)]
                libc::kill(id as i32, libc::SIGKILL);
            }
        }
    }
}

struct Dtrd {
    runner: DtrdRunner,
    client: dtrd_client::Client,
    #[allow(dead_code)]
    grpc_port: u16,
    tcpcl_port: u16,
    node_id: String,
    tmpdir: PathBuf,
    bundle_dir: PathBuf,
    allowed_messages: Vec<String>,
}

impl Dtrd {
    async fn new() -> Res<Self> {
        let port_range = PORT_COUNTER.fetch_add(10, std::sync::atomic::Ordering::SeqCst);
        let node_id = format!("dtn://testrunnode{port_range}");
        let grpc_port = port_range + 1;
        let tcpcl_port = port_range + 2;

        let mut tmpdir = std::env::temp_dir();
        tmpdir.push(format!("dtrd-ci-test-{port_range}"));
        let _ = fs::remove_dir_all(&tmpdir).await;

        let mut bundle_dir = tmpdir.clone();
        bundle_dir.push("bundles");
        fs::create_dir_all(&bundle_dir).await?;

        let runner = DtrdRunner::new(&node_id, grpc_port, tcpcl_port, &bundle_dir).await?;

        let client = dtrd_client::Client::new(&format!("http://127.0.0.1:{grpc_port}")).await?;

        Ok(Dtrd {
            runner,
            client,
            grpc_port,
            tcpcl_port,
            node_id,
            tmpdir,
            bundle_dir,
            allowed_messages: Vec::new(),
        })
    }

    async fn stop(&mut self) -> Res<()> {
        self.runner.stop(&self.allowed_messages).await
    }

    async fn restart(&mut self) -> Res<()> {
        self.runner
            .start(
                &self.node_id,
                self.grpc_port,
                self.tcpcl_port,
                &self.bundle_dir,
            )
            .await
    }

    fn allow_message(&mut self, msg: &str) {
        self.allowed_messages.push(msg.to_string());
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

impl Drop for Dtrd {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmpdir);
    }
}

async fn with_dtrds<F>(count: usize, func: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: AsyncFnOnce(Vec<&mut Dtrd>) -> Result<(), Box<dyn std::error::Error>>,
{
    let mut dtrds = Vec::with_capacity(count);
    for _ in 0..count {
        dtrds.push(Dtrd::new().await?);
    }
    let refs: Vec<&mut Dtrd> = dtrds.iter_mut().collect();
    let res = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(func(refs)))
        .timeout(Duration::from_secs(10))
        .await;

    let mut out = match res {
        Ok(Ok(result)) => result,
        Ok(Err(panic)) => Err(format!("PANIC: {panic:?}").into()),
        Err(_) => Err("timeout".into()),
    };

    for mut dtrd in dtrds {
        println!("\nStopping {}:", dtrd.grpc_port);
        if let Err(e) = dtrd.stop().await
            && out.is_ok()
        {
            out = Err(e);
        }
    }
    out
}

#[tokio::test]
async fn delivers_bundles_locally() -> Result<(), Box<dyn std::error::Error>> {
    with_dtrds(1, async |mut dtrds| {
        let dtrd = dtrds.remove(0);
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
        Ok(())
    })
    .await
}

#[tokio::test]
async fn delivers_bundles_connected() -> Result<(), Box<dyn std::error::Error>> {
    with_dtrds(2, async |mut dtrds| {
        let dtrd1 = dtrds.remove(0);
        let dtrd2 = dtrds.remove(0);
        dtrd1.connect_to(dtrd2).await?;
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
        Ok(())
    })
    .await
}

#[tokio::test]
async fn delivers_bundles_routed() -> Result<(), Box<dyn std::error::Error>> {
    with_dtrds(3, async |mut dtrds| {
        let dtrd1 = dtrds.remove(0);
        let dtrd2 = dtrds.remove(0);
        let dtrd3 = dtrds.remove(0);
        dtrd1.connect_to(dtrd2).await?;
        dtrd2.connect_to(dtrd3).await?;
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
        Ok(())
    })
    .await
}

#[tokio::test]
async fn hop_count_causes_expiry() -> Result<(), Box<dyn std::error::Error>> {
    const LOOP_NODE: &str = "dtn://thisnodedoesnotexist";
    with_dtrds(2, async |mut dtrds| {
        let dtrd1 = dtrds.remove(0);
        let dtrd2 = dtrds.remove(0);
        dtrd1.connect_to(dtrd2).await?;
        dtrd1
            .client
            .add_route(LOOP_NODE.to_string(), dtrd2.node_id.clone())
            .await?;
        dtrd2
            .client
            .add_route(LOOP_NODE.to_string(), dtrd1.node_id.clone())
            .await?;
        dtrd1
            .client
            .submit_bundle(LOOP_NODE, 60, DUMMY_DATA.as_bytes(), false)
            .await?;

        // We should now get a hop limit exceeded message back
        let data = dtrd1.client.receive_bundle(&dtrd1.node_id).await?;
        if let Ok(AdministrativeRecord::BundleStatusReport(bsr)) =
            AdministrativeRecord::try_from(data)
        {
            assert_eq!(bsr.reason, BundleStatusReason::HopLimitExceeded);
            assert!(bsr.status_information.deleted_bundle.is_asserted);
        } else {
            unreachable!();
        }

        // Allow this warning message
        dtrd2.allow_message("forwarding bundle failed: HopLimitExceeded");

        Ok(())
    })
    .await
}

#[tokio::test]
async fn bundle_stored_across_restarts() -> Result<(), Box<dyn std::error::Error>> {
    with_dtrds(1, async |mut dtrds| {
        let dtrd = dtrds.remove(0);
        dtrd.client
            .submit_bundle(
                &dtrd.with_node_id("testendpoint"),
                60,
                DUMMY_DATA.as_bytes(),
                false,
            )
            .await?;

        // Restart and check if the bundle is there afterwards
        dtrd.stop().await?;
        dtrd.restart().await?;

        let data = dtrd
            .client
            .receive_bundle(&dtrd.with_node_id("testendpoint"))
            .await?;
        assert_eq!(&String::from_utf8(data)?, DUMMY_DATA);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn delivers_bundles_fragmented() -> Result<(), Box<dyn std::error::Error>> {
    with_dtrds(2, async |mut dtrds| {
        let dtrd1 = dtrds.remove(0);
        let dtrd2 = dtrds.remove(0);
        dtrd1.connect_to(dtrd2).await?;

        // We now need to generate a bundle larger than the tcpcl max transfer
        // size. What is in there is something we can ignore.
        let target_size = tcpcl::v4::messages::sess_init::MAX_TRANSFER_MRU;
        let mut data = Vec::with_capacity(target_size);
        while data.len() < target_size {
            data.extend_from_slice(DUMMY_DATA.as_bytes());
        }

        dtrd1
            .client
            .submit_bundle(&dtrd2.with_node_id("testendpoint"), 60, &data, false)
            .await?;
        let received_data = dtrd2
            .client
            .receive_bundle(&dtrd2.with_node_id("testendpoint"))
            .await?;
        assert_eq!(data, received_data);
        Ok(())
    })
    .await
}
