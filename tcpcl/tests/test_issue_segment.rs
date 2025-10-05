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

use tcpcl::errors::{ErrorType, Errors};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::common::*;

mod common;

#[tokio::test]
async fn test_unkown_message_type() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write_all(&[
                0x01, // message type
                0x02, // flags (start)
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id
                0x00, 0x00, 0x00, 0x00, // transfer extensions
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // data bytes
                0x55, 0xAA, // fake data
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
    })
    .await?;

    let ret = session.manage_connection().await;
    assert!(matches!(
        ret,
        Err(ErrorType::TCPCLError(Errors::MessageError(
            tcpcl::v4::messages::Errors::SegmentTooLong
        )))
    ));
    jh.await.unwrap();

    Ok(())
}
