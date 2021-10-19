use crate::errors::Errors;

use super::reader::Reader;

pub trait Transform {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized;

    fn write(&self, target: &mut Vec<u8>);
}
