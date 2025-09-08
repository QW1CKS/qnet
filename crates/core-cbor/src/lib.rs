//! Deterministic CBOR helpers

use serde::Serialize;
use std::collections::BTreeMap;

// Encode any Serialize deterministically. For maps, prefer BTreeMap to ensure key order.
pub fn to_det_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_cbor::Error> {
    let mut ser = serde_cbor::ser::Serializer::new(Vec::new());
    ser.self_describe()?; // CBOR self-describe tag for clarity; consistent across runs
    value.serialize(&mut ser)?;
    Ok(ser.into_inner())
}

// Convenience: encode a BTreeMap (keys are ordered) to deterministic CBOR bytes.
pub fn encode_map<K, V>(map: &BTreeMap<K, V>) -> Result<Vec<u8>, serde_cbor::Error>
where
    K: Ord + Serialize,
    V: Serialize,
{
    to_det_cbor(map)
}

// TemplateID = SHA-256(DET-CBOR(params))
pub fn compute_template_id<T: Serialize>(params: &T) -> [u8; 32] {
    let bytes = to_det_cbor(params).expect("CBOR encode");
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(digest.as_ref());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Params {
        ver: u8,
        opts: BTreeMap<String, u64>,
    }

    #[test]
    fn det_cbor_map_order_and_template_id_stability() {
        let mut a = BTreeMap::new();
        a.insert("z".to_string(), 9);
        a.insert("a".to_string(), 1);
        a.insert("m".to_string(), 5);

        let mut b = BTreeMap::new();
        // insert in different order but same key/value set
        b.insert("m".to_string(), 5);
        b.insert("z".to_string(), 9);
        b.insert("a".to_string(), 1);

        let p1 = Params { ver: 1, opts: a };
        let p2 = Params { ver: 1, opts: b };

        let c1 = to_det_cbor(&p1).unwrap();
        let c2 = to_det_cbor(&p2).unwrap();
        assert_eq!(c1, c2, "deterministic bytes for same logical map");

        let id1 = compute_template_id(&p1);
        let id2 = compute_template_id(&p2);
        assert_eq!(id1, id2, "same TemplateID for equal params");
        assert!(id1.iter().any(|&b| b != 0));
    }

    #[test]
    fn template_id_changes_on_param_change() {
        let mut a = BTreeMap::new();
        a.insert("a".to_string(), 1);
        a.insert("b".to_string(), 2);
        let mut b = a.clone();
        b.insert("c".to_string(), 3); // changed

        let p1 = Params { ver: 1, opts: a };
        let p2 = Params { ver: 1, opts: b };

        let id1 = compute_template_id(&p1);
        let id2 = compute_template_id(&p2);
        assert_ne!(id1, id2, "TemplateID must change when params change");
    }
}
