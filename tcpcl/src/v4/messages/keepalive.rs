use crate::{
    errors::Errors,
    v4::{reader::Reader, transform::Transform},
};

#[derive(Debug)]
pub struct Keepalive {}

impl Keepalive {
    pub fn new() -> Self {
        Keepalive {}
    }
}

impl Transform for Keepalive {
    fn read(_reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized,
    {
        Ok(Keepalive {})
    }

    fn write(&self, _target: &mut Vec<u8>) {}
}
