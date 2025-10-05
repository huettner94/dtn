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

use tcpcl::errors::ErrorType;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::common::*;

mod common;

#[tokio::test]
async fn test_unkown_message_type() -> Result<(), ErrorType> {
    let (jh, mut session) = setup_conn(|mut client| async move {
        client
            .write_all(&[
                0xFF, // non existing message type
            ])
            .await
            .unwrap();

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 3);
        assert_eq!(
            buf[0..3],
            [
                0x06, // message type
                0x01, // reason code
                0xFF, // our message type
            ]
        );

        let mut buf: [u8; 100] = [0; 100];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
    })
    .await?;

    session.manage_connection().await.unwrap();
    jh.await.unwrap();

    Ok(())
}
