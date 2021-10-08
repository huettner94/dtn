use super::{errors::Errors, reader::Reader};

pub trait Transform {
    fn read(reader: &mut Reader) -> Result<Self, Errors>
    where
        Self: Sized;

    fn write(self, target: &mut Vec<u8>);
}
