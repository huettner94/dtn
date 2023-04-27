use bytes::BytesMut;

#[derive(Debug)]
pub struct Keepalive {}

impl Keepalive {
    pub fn new() -> Self {
        Keepalive {}
    }

    pub fn decode(_src: &mut BytesMut) -> Result<Option<Self>, crate::v4::messages::Errors> {
        Ok(Some(Keepalive {}))
    }

    pub fn encode(&self, _dst: &mut BytesMut) {}
}
