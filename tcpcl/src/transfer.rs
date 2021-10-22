#[derive(Debug, PartialEq, Eq)]
pub struct Transfer {
    pub id: u64,
    pub data: Vec<u8>,
}
