# QNet Catalog System Removal - Migration Notes

## Overview
**Date**: November 27, 2025  
**Change**: Removed catalog system entirely - switched to hardcoded operator exits + public libp2p DHT  
**Impact**: ~300 lines removed, $5/mo cost savings, faster startup, simpler architecture

---

## What Changed

### Architecture Before
```
Priority 1: Download signed catalog from catalog.qnet.io
Priority 2: IPFS DHT (fallback)
Priority 3: Hardcoded seeds (last resort)
```

### Architecture After
```
Priority 1: Hardcoded operator exit nodes (6 DigitalOcean droplets)
Priority 2: Public libp2p DHT (decentralized discovery)
```

---

## Technical Changes

### Files Modified
| File | Changes | Lines Removed |
|------|---------|---------------|
| `discovery.rs` | Removed catalog parsing functions | ~104 |
| `main.rs` | Removed `CatalogState` and all infrastructure | ~195 |
| **Total** | | **~300 lines** |

### Code Removed
- ✅ `CatalogState` struct and all impl methods
- ✅ `CatalogMeta`, `CatalogInner`, `CatalogJson`, `CatalogEntry` structs
- ✅ `UpdateInfo` struct
- ✅ `load_catalog_from_json()`, `verify_and_parse_catalog()`, `parse_catalog_entries()`
- ✅ `check_for_updates_now()` async function
- ✅ Catalog initialization and background updater tasks
- ✅ Catalog references in status API and HTML templates

### Dependencies
- `directories` crate: Still used (decoy catalogs)
- `chrono` crate: Still used (other timestamp needs)
- No dependencies removed (all still needed elsewhere)

---

## Benefits

### Cost Savings
| Item | Before | After | Savings |
|------|--------|-------|---------|
| Catalog hosting | $5/mo | $0 | **$5/mo** |
| Exit node droplets | $24-36/mo | $24-36/mo | $0 |
| **Total** | **$29-41/mo** | **$24-36/mo** | **$5/mo** |

###Performance Improvements
- **Faster Startup**: No catalog download/verification (~500ms saved)
- **Simpler Codebase**: ~300 lines removed = easier maintenance
- **Less Attack Surface**: No catalog signature verification complexity

### Architectural Simplification
```mermaid
graph LR
    A[Helper Starts] --> B[Load Hardcoded Seeds]
    B --> C[Connect to Public DHT]
    C --> D[Discover Peers]
    D --> E[Ready]
    
    style A fill:#b197fc
    style E fill:#51cf66
```

---

## Operator Guide

### Updating Hardcoded IPs

**When you deploy the 6 exit node droplets**, follow these steps:

1. **Deploy Droplets**  
   Follow [exit_node_deployment.md](file:///p:/GITHUB/qnet/docs/exit_node_deployment.md)

2. **Extract Peer IDs**  
   ```bash
   ssh root@<DROPLET_IP>
   journalctl -u qnet-exit | grep "Generated local peer ID"
   ```

3. **Update Source Code**  
   Edit [discovery.rs](file:///p:/GITHUB/qnet/crates/core-mesh/src/discovery.rs#L143-L190):
   ```rust
   fn qnet_operator_seeds() -> Vec<BootstrapNode> {
       vec![
           BootstrapNode::new(
               "12D3KooW<actual_peer_id>".parse().unwrap(),
               "/ip4/<actual_ip>/tcp/4001".parse().unwrap(),
           ),
           // ... 5 more droplets
       ]
   }
   ```

4. **Build and Release**  
   ```bash
   cargo build --release -p stealth-browser
   # Upload to GitHub releases
   ```

5. **Users Update**  
   Download new binary from GitHub releases

---

## When to Reconsider Catalog

The catalog system was removed for simplicity, but may be worth reconsidering if:

### Scenario 1: Network Scale
**Trigger**: Network grows beyond 100 operator-controlled nodes  
**Reason**: Hardcoding 100+ IPs in source code becomes unwieldy  
**Solution**: Reintroduce catalog for dynamic peer lists

### Scenario 2: Community Growth
**Trigger**: Significant community volunteer exits emerge  
**Reason**: Need trust mechanism for community-run nodes  
**Solution**: Signed catalog with operator pubkey validation

### Scenario 3: Geographic Routing
**Trigger**: Need frequent metadata updates (latency, available bandwidth, etc.)  
**Reason**: Binary releases too slow for dynamic routing decisions  
**Solution**: Catalog with node metadata (region, capacity, health)

### Scenario 4: Regulatory Pressure
**Trigger**: Governments start blocking operator exit IPs  
**Reason**: Need dynamic IP rotation without binary redistribution  
**Solution**: Catalog + signed updates deployed via CDN

### Current Assessment (Nov 2025)
- ✅ **Small network** (~6 operator exits = hardcoded is fine)
- ✅ **Simple needs** (bootstrap-only, no complex routing)
- ✅ **Stable infrastructure** (DigitalOcean IPs rarely change)
- ✅ **Low cost priority** (every $5/mo saved matters for side project)

**Recommendation**: Keep hardcoded approach until network scales 10x

---

## Rollback Plan

If you need to revert this change:

1. **Restore from Git**  
   ```bash
   git revert <commit_hash>
   ```

2. **Key commits to revert** (search git log):
   - Catalog code removal from `discovery.rs`
   - `CatalogState` removal from `main.rs`
   - Documentation updates

3. **Rebuild catalog infrastructure**:
   - Regenerate signed catalog (use `catalog-signer` crate)
   - Deploy catalog.qnet.io (S3 + CloudFront)
   - Update environment variables with pubkey

4. **Timeline**: ~2 hours for experienced developer

---

## Security Considerations

### Before (Catalog)
**Attack Surface**:
- Catalog signing key compromise → inject malicious peers
- Catalog hosting compromise → serve unsigned/expired catalog
- Sigmat verification bugs → bypass security

**Mitigations**:
- Ed25519 signature verification
- Expiration timestamps
- Fallback to hardcoded seeds

### After (Hardcoded)
**Attack Surface**:
- Source code compromise → inject malicious IPs (requires attacker to compromise GitHub)
- Build system compromise → trojan binary (same risk as before)

**Mitigations**:
- Git commit signing
- Binary signature verification (future)
- Public libp2p DHT as fallback (trustless)

**Net Security Impact**: **Neutral to slightly positive** (less moving parts)

---

## Testing Checklist

After applying this change, verify:

- [ ] `cargo build --workspace` succeeds
- [ ] No `CatalogState` references remain (`grep -r "CatalogState" crates/ apps/`)
- [ ] No `catalog_version` in status API output
- [ ] Bootstrap discovery works with public DHT nodes
- [ ] Mesh peer discovery completes (check `peers_online` in status)
- [ ] Status HTML page loads without JavaScript errors

---

## Future Work

### Phase 4 (Testing) - Remaining
- [x] Build verification
- [x] Remove catalog code references
- [ ] Live testing with actual peer discovery (manual)

### Phase 5 (Documentation) - Complete
- [x] Migration notes created
- [x] Deployment guide created
- [x] README.md updated
- [x] Walkthrough documented

### Potential Enhancements
1. **Binary Signing**: Add GPG/Minisign signatures to GitHub releases
2. **Auto-Update**: Implement in-app update checker (download new binary)
3. **Health Monitoring**: Add `/health` endpoint to exit nodes
4. **Metrics Dashboard**: Track exit node uptime, bandwidth, peer count

---

## Questions?

**For operators deploying exit nodes**:  
See [exit_node_deployment.md](file:///p:/GITHUB/qnet/docs/exit_node_deployment.md)

**For developers**:  
- Code: [discovery.rs](file:///p:/GITHUB/qnet/crates/core-mesh/src/discovery.rs)
- Architecture: [README.md](file:///p:/GITHUB/qnet/README.md)

**For troubleshooting**:  
Check status API: `http://localhost:8088/status`
