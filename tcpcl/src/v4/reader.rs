use std::string::FromUtf8Error;

use tokio::{
    io::{self},
    net::TcpStream,
};

pub const READER_BUFFER_SIZE: usize = 10240;

#[derive(Debug)]
pub struct Reader {
    data: [u8; READER_BUFFER_SIZE],
    used: usize,
    read_pos: usize,
}

impl Reader {
    pub fn new() -> Self {
        Reader {
            data: [0; READER_BUFFER_SIZE],
            used: 0,
            read_pos: 0,
        }
    }

    pub async fn read(&mut self, reader: &mut TcpStream) -> io::Result<usize> {
        let read = reader.try_read(&mut self.data[self.used..])?;
        self.used += read;

        Ok(read)
    }

    pub fn read_u8(&mut self) -> u8 {
        if self.left() < 1 {
            panic!("Attempted to read beyond buffer!");
        }
        let val = self.data[self.read_pos];
        self.read_pos += 1;
        val
    }

    pub fn read_u16(&mut self) -> u16 {
        if self.left() < 2 {
            panic!("Attempted to read beyond buffer!");
        }
        let mut data: [u8; 2] = [0; 2];
        self.read_u8_array(&mut data, 2);
        u16::from_be_bytes(data)
    }

    pub fn read_u32(&mut self) -> u32 {
        if self.left() < 4 {
            panic!("Attempted to read beyond buffer!");
        }
        let mut data: [u8; 4] = [0; 4];
        self.read_u8_array(&mut data, 4);
        u32::from_be_bytes(data)
    }

    pub fn read_u64(&mut self) -> u64 {
        if self.left() < 8 {
            panic!("Attempted to read beyond buffer!");
        }
        let mut data: [u8; 8] = [0; 8];
        self.read_u8_array(&mut data, 8);
        u64::from_be_bytes(data)
    }

    pub fn read_u8_array(&mut self, target: &mut [u8], count: usize) {
        if self.left() < count {
            panic!("Attempted to read beyond buffer!");
        }
        for i in 0..count {
            target[i] = self.data[self.read_pos + i];
        }
        self.read_pos += count;
    }

    pub fn read_string(&mut self, len: usize) -> Result<String, FromUtf8Error> {
        if self.left() < len {
            panic!("Attempted to read beyond buffer!");
        }
        let out = String::from_utf8(self.data[self.read_pos..self.read_pos + len].to_vec());
        self.read_pos += len;
        return out;
    }

    pub fn current_pos(&self) -> usize {
        self.read_pos
    }

    pub fn left(&self) -> usize {
        self.used - self.read_pos
    }

    pub fn consume(&mut self) {
        self.data.copy_within(self.read_pos..self.used, 0);
        self.used -= self.read_pos;
        self.read_pos = 0;
    }

    pub fn reset_read(&mut self) {
        self.read_pos = 0;
    }
}
