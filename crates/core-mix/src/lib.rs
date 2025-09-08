use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub [u8;32]);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconSet {
    pub epoch: u64,
    pub nodes: Vec<NodeId>,
}

#[derive(Debug, Clone)]
pub struct DiversityTracker {
    // Track pairs (src,dst,epoch) => selected indices to avoid reuse until threshold
    recent: std::collections::VecDeque<(NodeId, NodeId, u64, usize)>,
    max_keep: usize,
}

impl DiversityTracker {
    pub fn new(max_keep: usize) -> Self { Self { recent: std::collections::VecDeque::new(), max_keep } }
    pub fn record(&mut self, src: &NodeId, dst: &NodeId, epoch: u64, idx: usize) {
        self.recent.push_back((src.clone(), dst.clone(), epoch, idx));
        while self.recent.len() > self.max_keep { self.recent.pop_front(); }
    }
    pub fn seen(&self, src: &NodeId, dst: &NodeId, epoch: u64, idx: usize) -> bool {
        self.recent.iter().any(|(s,d,e,i)| s==src && d==dst && *e==epoch && *i==idx)
    }
}

pub fn vrf_select(src: &NodeId, dst: &NodeId, epoch: u64, set: &BeaconSet) -> Option<usize> {
    if set.nodes.is_empty() { return None; }
    let mut h = Sha256::new();
    h.update(src.0);
    h.update(dst.0);
    h.update(epoch.to_le_bytes());
    let seed = h.finalize();
    // Derive deterministic index
    let idx = ((seed[0] as usize) << 8 | (seed[1] as usize)) % set.nodes.len();
    Some(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> NodeId { let mut a=[0u8;32]; a[0]=n; NodeId(a) }

    #[test]
    fn deterministic_selection() {
        let set = BeaconSet { epoch: 1, nodes: vec![id(1), id(2), id(3), id(4)] };
        let a = id(9); let b = id(5);
        let i1 = vrf_select(&a, &b, 42, &set).unwrap();
        let i2 = vrf_select(&a, &b, 42, &set).unwrap();
        assert_eq!(i1, i2);
    }

    #[test]
    fn diversity_tracking() {
        let mut dt = DiversityTracker::new(8);
        let set = BeaconSet { epoch: 7, nodes: vec![id(1), id(2), id(3)] };
        let a = id(10); let b = id(11);
        let idx = vrf_select(&a, &b, set.epoch, &set).unwrap();
        assert!(!dt.seen(&a,&b,set.epoch,idx));
        dt.record(&a,&b,set.epoch,idx);
        assert!(dt.seen(&a,&b,set.epoch,idx));
    }
}
