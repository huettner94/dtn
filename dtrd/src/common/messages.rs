use actix::prelude::*;

#[derive(Message, Debug)]
#[rtype(result = "")]
pub struct Shutdown {}
