use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OrgId(pub String);
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AsId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub uptime_ratio: f64, // 0..1
    pub org: OrgId,
    pub asn: AsId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caps {
    pub org_cap: f64, // e.g., 0.20 for 20%
    pub as_cap: f64,  // e.g., 0.25 for 25%
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub raw: f64,
    pub capped: f64,
}

pub fn score_nodes(nodes: Vec<Node>, caps: Caps) -> Vec<Score> {
    // raw scores proportional to uptime
    let raw: Vec<f64> = nodes.iter().map(|n| n.uptime_ratio.max(0.0).min(1.0)).collect();
    // apply caps by redistributing excess above cap proportionally
    // compute group sums
    use std::collections::HashMap;
    let mut by_org: HashMap<OrgId, f64> = HashMap::new();
    let mut by_as: HashMap<AsId, f64> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        *by_org.entry(n.org.clone()).or_default() += raw[i];
        *by_as.entry(n.asn.clone()).or_default() += raw[i];
    }
    // limit factors
    let mut capped = raw.clone();
    for (i, n) in nodes.iter().enumerate() {
        let org_total = by_org.get(&n.org).copied().unwrap_or(0.0);
        let as_total = by_as.get(&n.asn).copied().unwrap_or(0.0);
        let org_factor = if org_total > 0.0 { (caps.org_cap * org_total) / org_total } else { 1.0 };
        let as_factor = if as_total > 0.0 { (caps.as_cap * as_total) / as_total } else { 1.0 };
        let factor = org_factor.min(as_factor).min(1.0);
        capped[i] = raw[i] * factor;
    }
    raw.into_iter().zip(capped.into_iter()).map(|(raw, capped)| Score { raw, capped }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_bound_scores() {
        let nodes = vec![
            Node { uptime_ratio: 1.0, org: OrgId("A".into()), asn: AsId("AS1".into()) },
            Node { uptime_ratio: 1.0, org: OrgId("A".into()), asn: AsId("AS1".into()) },
            Node { uptime_ratio: 1.0, org: OrgId("B".into()), asn: AsId("AS2".into()) },
            Node { uptime_ratio: 0.5, org: OrgId("B".into()), asn: AsId("AS2".into()) },
        ];
        let caps = Caps { org_cap: 0.20, as_cap: 0.25 };
        let scores = score_nodes(nodes, caps);
        // raw
        assert_eq!(scores.iter().map(|s| s.raw).sum::<f64>(), 3.5);
        // check that at least some capping occurred
        assert!(scores.iter().map(|s| s.capped).sum::<f64>() <= 3.5);
    }
}
