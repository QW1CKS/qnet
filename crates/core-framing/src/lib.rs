//! L2 frame types (skeleton)

use bytes::{BufMut, Bytes, BytesMut};

#[derive(Debug, Clone, Copy)]
pub enum FrameType {
    Stream = 0x10,
    WindowUpdate = 0x11,
    Ping = 0x12,
    KeyUpdate = 0x13,
    Close = 0x1F,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub ty: FrameType,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn encode(&self) -> Bytes {
        let mut b = BytesMut::with_capacity(4 + self.payload.len());
        // Len(u24) | Type(u8) | payload
        let len = self.payload.len() as u32 + 1; // include type
        b.put_u8(((len >> 16) & 0xff) as u8);
        b.put_u8(((len >> 8) & 0xff) as u8);
        b.put_u8((len & 0xff) as u8);
        b.put_u8(self.ty as u8);
        b.extend_from_slice(&self.payload);
        b.freeze()
    }
}
