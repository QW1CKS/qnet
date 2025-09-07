//! HTX crate (skeleton)

pub struct Client;
pub struct Server;

impl Client {
    pub fn dial(_addr: &str) -> Result<(), &'static str> { Ok(()) }
}

impl Server {
    pub fn accept(_bind: &str) -> Result<(), &'static str> { Ok(()) }
}
