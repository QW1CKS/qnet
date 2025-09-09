use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

type Signer = [u8; 32];
type Votes = BTreeSet<Signer>;
type Seq = u64;
type PendingBySeq = HashMap<Seq, (Entry, Votes)>;
type Pending = HashMap<Alias, PendingBySeq>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Alias([u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId([u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub alias: Alias,
    pub target: PeerId,
    pub ts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumCert {
    pub signers: BTreeSet<[u8; 32]>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sequence too old")]
    OldSeq,
    #[error("conflict not resolved")]
    Conflict,
    #[error("insufficient quorum")]
    NoQuorum,
}

#[derive(Default)]
pub struct Ledger {
    // per-alias latest committed entry
    committed: Mutex<HashMap<Alias, Entry>>,
    // pending entries per alias (seq -> (entry, votes))
    pending: Mutex<Pending>,
    // emergency lock: if set, only entries with signer in set may advance without quorum
    emergency: Mutex<Option<BTreeSet<[u8; 32]>>>,
    quorum: usize,
}

impl Ledger {
    pub fn new(quorum: usize) -> Self {
        Self {
            quorum,
            ..Default::default()
        }
    }

    pub fn propose(&self, alias: Alias, target: PeerId, seq: u64) -> Entry {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Entry {
            seq,
            alias,
            target,
            ts,
        }
    }

    pub fn vote(&self, e: &Entry, signer: [u8; 32]) -> Result<(), Error> {
        // reject votes for stale sequences
        if let Some(comm) = self.committed.lock().get(&e.alias) {
            if e.seq <= comm.seq {
                return Err(Error::OldSeq);
            }
        }
        let mut p = self.pending.lock();
        let by_alias = p.entry(e.alias).or_default();
        let (entry, voters) = by_alias
            .entry(e.seq)
            .or_insert_with(|| (e.clone(), BTreeSet::new()));
        if entry != e {
            return Err(Error::Conflict);
        }
        voters.insert(signer);
        Ok(())
    }

    pub fn try_commit(&self, alias: Alias) -> Result<Option<Entry>, Error> {
        let mut p = self.pending.lock();
        let Some(by_seq) = p.get_mut(&alias) else {
            return Ok(None);
        };
        // pick highest seq with quorum
        let mut best: Option<(u64, Entry)> = None;
        for (seq, (e, voters)) in by_seq.iter() {
            if voters.len() >= self.quorum && best.as_ref().map(|(s, _)| seq > s).unwrap_or(true) {
                best = Some((*seq, e.clone()));
            }
        }
        if let Some((seq, e)) = best {
            self.committed.lock().insert(alias, e.clone());
            by_seq.retain(|s, _| *s > seq); // keep only higher seqs
            return Ok(Some(e));
        }
        // emergency path: if emergency set exists, allow single authorized signer to advance highest seq
        if let Some(allow) = self.emergency.lock().clone() {
            let mut chosen: Option<(u64, Entry)> = None;
            for (seq, (e, voters)) in by_seq.iter() {
                if voters.iter().any(|v| allow.contains(v))
                    && chosen.as_ref().map(|(s, _)| seq > s).unwrap_or(true)
                {
                    chosen = Some((*seq, e.clone()));
                }
            }
            if let Some((seq, e)) = chosen {
                self.committed.lock().insert(alias, e.clone());
                by_seq.retain(|s, _| *s > seq);
                return Ok(Some(e));
            }
        }
        Ok(None)
    }

    pub fn set_emergency(&self, allow: Option<BTreeSet<[u8; 32]>>) {
        *self.emergency.lock() = allow;
    }

    pub fn head(&self, alias: &Alias) -> Option<Entry> {
        self.committed.lock().get(alias).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> [u8; 32] {
        let mut a = [0u8; 32];
        a[0] = n;
        a
    }

    #[test]
    fn quorum_commit() {
        let ledger = Ledger::new(2);
        let alias = Alias([1u8; 32]);
        let p1 = PeerId([9u8; 32]);
        let e = ledger.propose(alias, p1, 1);
        ledger.vote(&e, id(1)).unwrap();
        ledger.vote(&e, id(2)).unwrap();
        let committed = ledger.try_commit(alias).unwrap();
        assert!(committed.is_some());
        assert_eq!(ledger.head(&alias).unwrap().target, p1);
    }

    #[test]
    fn conflict_requires_resolution() {
        let ledger = Ledger::new(2);
        let alias = Alias([2u8; 32]);
        let p1 = PeerId([1u8; 32]);
        let p2 = PeerId([2u8; 32]);
        let e1 = ledger.propose(alias, p1, 5);
        let e2 = ledger.propose(alias, p2, 5);
        ledger.vote(&e1, id(1)).unwrap();
        // conflict: different target with same seq
        assert!(matches!(ledger.vote(&e2, id(2)), Err(Error::Conflict)));
        // need matching entry votes
        ledger.vote(&e1, id(3)).unwrap();
        let committed = ledger.try_commit(alias).unwrap();
        assert!(committed.is_some());
        assert_eq!(ledger.head(&alias).unwrap().target, p1);
    }

    #[test]
    fn emergency_path_advances() {
        let ledger = Ledger::new(2);
        let alias = Alias([3u8; 32]);
        let p1 = PeerId([7u8; 32]);
        let e = ledger.propose(alias, p1, 10);
        ledger.vote(&e, id(9)).unwrap(); // only one vote
                                         // set emergency allowing signer 9
        let mut allow = BTreeSet::new();
        allow.insert(id(9));
        ledger.set_emergency(Some(allow));
        let committed = ledger.try_commit(alias).unwrap();
        assert!(committed.is_some());
        assert_eq!(ledger.head(&alias).unwrap().target, p1);
    }
}
