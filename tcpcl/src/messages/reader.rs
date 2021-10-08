use log::{debug, info};
use tokio::{
    io::{self, AsyncReadExt},
    net::TcpStream,
};

const READER_BUFFER_SIZE: usize = 10240;

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

    pub fn read_u8_array(&mut self, target: &mut [u8], count: usize) {
        if self.left() < count {
            panic!("Attempted to read beyond buffer!");
        }
        for i in 0..count {
            target[i] = self.data[self.read_pos + i];
        }
        self.read_pos += count;
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
