pub mod responder;

pub const CONTAINER_COMMAND: u16 = 1;
pub const CONTAINER_DATA: u16 = 2;
pub const CONTAINER_RESPONSE: u16 = 3;

pub const STORAGE_ID: u32 = 0x0001_0001;
pub const ROOT_PARENT: u32 = 0xFFFF_FFFF;

pub trait Transport {
    type Error;
    async fn read_packet(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
    async fn write_packet(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
}
