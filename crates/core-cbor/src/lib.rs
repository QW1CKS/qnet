//! Deterministic CBOR helpers

use serde::Serialize;

pub fn to_det_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_cbor::Error> {
    let mut ser = serde_cbor::ser::Serializer::new(Vec::new());
    ser.self_describe() ?; // tag
    ser.set_sort_keys(true);
    ser.set_canonical(true);
    value.serialize(&mut ser)?;
    Ok(ser.into_inner())
}
