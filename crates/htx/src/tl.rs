use core_framing as framing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatMode {
    Native,
    V1_1,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyUpdatePolicy {
    pub max_frames: usize,
    pub max_seconds: u64,
}

#[derive(Debug, Default, Clone)]
pub struct PolicyTracker {
    frames_since_update: usize,
    last_update_ts: u64,
}

impl PolicyTracker {
    pub fn new(now: u64) -> Self {
        Self {
            frames_since_update: 0,
            last_update_ts: now,
        }
    }
    pub fn on_frame_sent(&mut self, _frame: &framing::Frame) {
        self.frames_since_update = self.frames_since_update.saturating_add(1);
    }
    pub fn mark_updated(&mut self, now: u64) {
        self.frames_since_update = 0;
        self.last_update_ts = now;
    }
}

pub fn should_key_update(policy: KeyUpdatePolicy, tracker: &PolicyTracker, now: u64) -> bool {
    if policy.max_frames > 0 && tracker.frames_since_update >= policy.max_frames {
        return true;
    }
    if policy.max_seconds > 0 && now.saturating_sub(tracker.last_update_ts) >= policy.max_seconds {
        return true;
    }
    false
}

// For PoC, v1.1 mapping is identity; this is a hook for future on-the-wire differences.
pub fn map_frame_to_v11(frame: &framing::Frame) -> framing::Frame {
    framing::Frame {
        ty: frame.ty,
        payload: frame.payload.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_mapping_keeps_payload() {
        let f = framing::Frame {
            ty: framing::FrameType::Stream,
            payload: vec![1, 2, 3],
        };
        let g = map_frame_to_v11(&f);
        assert_eq!(g.payload, f.payload);
        assert_eq!(g.ty as u8, f.ty as u8);
    }

    #[test]
    fn policy_triggers_on_limits() {
        let mut tr = PolicyTracker::new(100);
        let pol = KeyUpdatePolicy {
            max_frames: 3,
            max_seconds: 10,
        };
        let f = framing::Frame {
            ty: framing::FrameType::Ping,
            payload: vec![],
        };
        for _ in 0..2 {
            tr.on_frame_sent(&f);
        }
        assert!(!should_key_update(pol, &tr, 105));
        tr.on_frame_sent(&f);
        assert!(should_key_update(pol, &tr, 105));
        // After marking updated, only seconds limit should trip
        tr.mark_updated(105);
        assert!(!should_key_update(pol, &tr, 110));
        assert!(should_key_update(pol, &tr, 116));
    }
}
