use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid length: expected 128 bytes, got {0}")]
    InvalidLength(usize),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Voucher([u8; 128]);

impl std::fmt::Debug for Voucher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Voucher({}..)", self.0.len())
    }
}

impl Voucher {
    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        if b.len() != 128 {
            return Err(Error::InvalidLength(b.len()));
        }
        let mut arr = [0u8; 128];
        arr.copy_from_slice(b);
        Ok(Self(arr))
    }
    pub fn as_bytes(&self) -> &[u8; 128] {
        &self.0
    }
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
    pub fn from_hex(s: &str) -> Result<Self, Error> {
        let v = hex::decode(s).map_err(|_| Error::InvalidLength(s.len() / 2))?;
        Self::from_bytes(&v)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatePlaceholder {
    pub count: u32,
}

impl AggregatePlaceholder {
    pub fn add(&mut self, _v: &Voucher) {
        self.count = self.count.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let v = [7u8; 128];
        let voucher = Voucher::from_bytes(&v).unwrap();
        assert_eq!(voucher.as_bytes(), &v);
        let h = voucher.to_hex();
        let voucher2 = Voucher::from_hex(&h).unwrap();
        assert_eq!(voucher, voucher2);
    }

    #[test]
    fn invalid_len() {
        assert!(Voucher::from_bytes(&[0u8; 127]).is_err());
        assert!(Voucher::from_bytes(&[0u8; 129]).is_err());
    }

    #[test]
    fn aggregate_placeholder_increments() {
        let mut a = AggregatePlaceholder::default();
        let v = Voucher::from_bytes(&[1u8; 128]).unwrap();
        a.add(&v);
        a.add(&v);
        assert_eq!(a.count, 2);
    }
}
