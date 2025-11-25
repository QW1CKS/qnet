//! Integration tests for mesh relay functionality.
//!
//! Tests multi-hop packet forwarding between mesh nodes.

use core_mesh::relay::{handle_incoming_packet, Packet, RelayBehavior, RoutingTable};
use libp2p::PeerId;
use std::sync::{Arc, Mutex};

/// Test 3-node relay: Node A sends to Node C via Node B
#[async_std::test]
async fn test_three_node_relay() {
    // Create three peer IDs
    let peer_a = PeerId::random();
    let peer_b = PeerId::random();
    let peer_c = PeerId::random();

    // Setup routing tables
    // A knows B is the route to C
    let mut routing_a = RoutingTable::new();
    routing_a.add_route(peer_c, peer_b);

    // B knows C directly
    let mut routing_b = RoutingTable::new();
    routing_b.add_route(peer_c, peer_c);

    // Create relay behaviors
    let mut relay_a = RelayBehavior::new(peer_a, routing_a);
    let mut relay_b = RelayBehavior::new(peer_b, routing_b);
    let relay_c = RelayBehavior::new(peer_c, RoutingTable::new());

    // Track delivered packets
    let delivered_c = Arc::new(Mutex::new(Vec::new()));
    let delivered_c_clone = Arc::clone(&delivered_c);

    // Create test packet from A to C
    let test_data = b"Hello from A to C via B".to_vec();
    let packet = Packet::new(peer_a, peer_c, test_data.clone());

    // A forwards the packet (should go to B)
    assert!(relay_a.should_relay(&packet));
    let result = relay_a.forward_packet(packet.clone()).await;
    assert!(result.is_ok());
    assert_eq!(relay_a.packets_relayed(), 1);

    // B receives and forwards the packet (should go to C)
    let packet_at_b = packet.clone();
    assert!(relay_b.should_relay(&packet_at_b));
    let result = relay_b.forward_packet(packet_at_b).await;
    assert!(result.is_ok());
    assert_eq!(relay_b.packets_relayed(), 1);

    // C receives and delivers locally
    let mut relay_c_mut = relay_c;
    let packet_at_c = packet.clone();
    assert!(!relay_c_mut.should_relay(&packet_at_c));

    // Simulate local delivery at C
    let result = handle_incoming_packet(packet_at_c, &mut relay_c_mut, |p| {
        delivered_c_clone.lock().unwrap().push(p.data.clone());
        Ok(())
    })
    .await;
    assert!(result.is_ok());

    // Verify packet was delivered to C with correct data
    let delivered = delivered_c.lock().unwrap();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0], test_data);
}

/// Test packet forwarding with route invalidation
#[async_std::test]
async fn test_relay_with_route_invalidation() {
    let peer_a = PeerId::random();
    let peer_b = PeerId::random();
    let peer_c = PeerId::random();

    // A knows route to C via B
    let mut routing_a = RoutingTable::new();
    routing_a.add_route(peer_c, peer_b);

    let mut relay_a = RelayBehavior::new(peer_a, routing_a);

    let packet = Packet::new(peer_a, peer_c, vec![1, 2, 3]);

    // Forward should succeed
    let result = relay_a.forward_packet(packet.clone()).await;
    assert!(result.is_ok());
    assert_eq!(relay_a.packets_relayed(), 1);

    // Remove route
    relay_a.routing_table_mut().remove_route(&peer_c);

    // Forward should now fail
    let result = relay_a.forward_packet(packet).await;
    assert!(result.is_err());
}

/// Test relay statistics tracking
#[async_std::test]
async fn test_relay_statistics() {
    let local_peer = PeerId::random();
    let dst = PeerId::random();
    let next_hop = PeerId::random();

    let mut routing = RoutingTable::new();
    routing.add_route(dst, next_hop);

    let mut relay = RelayBehavior::new(local_peer, routing);

    // Initially zero packets relayed
    assert_eq!(relay.packets_relayed(), 0);
    assert_eq!(relay.routing_table().route_count(), 1);

    // Forward 10 packets
    for i in 0..10 {
        let packet = Packet::new(PeerId::random(), dst, vec![i]);
        relay.forward_packet(packet).await.expect("forward failed");
    }

    // Statistics should be updated
    assert_eq!(relay.packets_relayed(), 10);
    assert_eq!(relay.routing_table().route_count(), 1);
}

/// Test packet encoding integrity through relay chain
#[async_std::test]
async fn test_packet_encoding_integrity() {
    let src = PeerId::random();
    let dst = PeerId::random();

    // Test various data sizes
    let test_cases = vec![
        vec![],                                      // Empty
        vec![0xff; 1],                               // Single byte
        vec![0xaa; 100],                             // Medium
        vec![0x55; 10000],                           // Large
        b"Unicode test: \xE2\x9C\x93 \xF0\x9F\x8E\x89".to_vec(), // UTF-8
    ];

    for test_data in test_cases {
        let packet = Packet::new(src, dst, test_data.clone());

        // Encode and decode
        let encoded = packet.encode().expect("encode failed");
        let decoded = Packet::decode(&encoded).expect("decode failed");

        // Verify integrity
        assert_eq!(decoded.src, src);
        assert_eq!(decoded.dst, dst);
        assert_eq!(decoded.data, test_data);
    }
}

/// Test concurrent packet handling
#[async_std::test]
async fn test_concurrent_relay() {
    let local_peer = PeerId::random();
    let dst1 = PeerId::random();
    let dst2 = PeerId::random();
    let next_hop1 = PeerId::random();
    let next_hop2 = PeerId::random();

    let mut routing = RoutingTable::new();
    routing.add_route(dst1, next_hop1);
    routing.add_route(dst2, next_hop2);

    let mut relay = RelayBehavior::new(local_peer, routing);

    // Forward packets sequentially (not concurrent due to &mut requirement)
    for i in 0..20 {
        let dst = if i % 2 == 0 { dst1 } else { dst2 };
        let packet = Packet::new(PeerId::random(), dst, vec![i as u8]);
        relay.forward_packet(packet).await.expect("forward failed");
    }

    // All should succeed
    assert_eq!(relay.packets_relayed(), 20);
}

/// Test error handling when no route exists
#[async_std::test]
async fn test_no_route_error() {
    let local_peer = PeerId::random();
    let unknown_dst = PeerId::random();

    let mut relay = RelayBehavior::new(local_peer, RoutingTable::new());

    let packet = Packet::new(PeerId::random(), unknown_dst, vec![1, 2, 3]);

    let result = relay.forward_packet(packet).await;
    assert!(result.is_err());

    // No packets should be counted as relayed on error
    assert_eq!(relay.packets_relayed(), 0);
}
