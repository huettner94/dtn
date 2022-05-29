use std::fmt::Debug;

#[derive(PartialEq, Eq)]
pub struct Transfer {
    pub id: u64,
    pub data: Vec<u8>,
}

impl Debug for Transfer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transfer")
            .field("id", &self.id)
            .field("data (length)", &self.data.len())
            .finish()
    }
}
