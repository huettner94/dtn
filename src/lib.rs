pub mod block;
pub mod blockflags;
pub mod bundle;
pub mod bundleflags;
pub mod crc;
pub mod endpoint;
pub mod primaryblock;
pub mod time;

pub trait Validate {
    fn validate(&self) -> bool;
}
