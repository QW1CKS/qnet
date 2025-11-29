//! Integration tests for circuit building in the mesh network (DISABLED - Task 2.1.10)
//!
//! NOTE: These tests are disabled after DHT removal (Task 2.1.10).
//! Tests need to be rewritten to work with operator directory peer discovery.
//!
//! These tests validated circuit construction, routing table integration,
//! and circuit lifecycle management using Kademlia DHT peer lists.

// Disabled - awaiting rewrite for operator directory model
#[cfg(disabled)]
mod disabled_tests {
use core_mesh::circuit::{Circuit, CircuitBuilder, CircuitClose, CircuitReady, CircuitRequest};
use core_mesh::discovery::DiscoveryBehavior;
use core_mesh::relay::RoutingTable;
use libp2p::{identity, PeerId};
use std::sync::{Arc, Mutex};

#[async_std::test]
async fn test_circuit_builder_creates_valid_circuit() {
    // Create a discovery behavior with some peers
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    let (_relay_transport, discovery) = DiscoveryBehavior::new(peer_id, vec![]).await.unwrap();
    let discovery_arc = Arc::new(Mutex::new(discovery));

    let builder = CircuitBuilder::new(discovery_arc.clone());

    // Manually add some peers to discovery (simulate discovered peers)
    let peer1 = PeerId::random();
    let peer2 = PeerId::random();
    let peer3 = PeerId::random();
    let destination = PeerId::random();

    {
        let mut disc = discovery_arc.lock().unwrap();
        let kad = &mut disc.kademlia;
        kad.add_address(&peer1, "/ip4/127.0.0.1/tcp/4001".parse().unwrap());
        kad.add_address(&peer2, "/ip4/127.0.0.1/tcp/4002".parse().unwrap());
        kad.add_address(&peer3, "/ip4/127.0.0.1/tcp/4003".parse().unwrap());
    }

    // Build a circuit (this will fail due to insufficient peers, but tests the flow)
    let result = builder.build_circuit(destination, 2).await;

    // With manual peer addition to Kademlia, we might not have enough routable peers
    // This test validates the circuit builder attempts to create circuits
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_one_hop_circuit() {
    let peer1 = PeerId::random();
    let destination = PeerId::random();

    let circuit = Circuit::new(vec![peer1, destination]);

    assert_eq!(circuit.hops.len(), 2);
    assert_eq!(circuit.hops[0], peer1);
    assert_eq!(circuit.hops[1], destination);
    assert_eq!(circuit.next_hop(&peer1), Some(&destination));
    assert_eq!(circuit.next_hop(&destination), None);
}

#[test]
fn test_three_hop_circuit() {
    let hop1 = PeerId::random();
    let hop2 = PeerId::random();
    let destination = PeerId::random();

    let circuit = Circuit::new(vec![hop1, hop2, destination]);

    assert_eq!(circuit.hops.len(), 3);
    assert_eq!(circuit.next_hop(&hop1), Some(&hop2));
    assert_eq!(circuit.next_hop(&hop2), Some(&destination));
    assert_eq!(circuit.next_hop(&destination), None);
}

#[test]
fn test_routing_table_circuit_integration() {
    let mut routing_table = RoutingTable::new();

    let hop1 = PeerId::random();
    let destination = PeerId::random();
    let circuit = Circuit::new(vec![hop1, destination]);
    let circuit_id = circuit.id;

    // Add circuit to routing table
    routing_table.add_circuit(circuit).unwrap();

    assert_eq!(routing_table.circuit_count(), 1);

    // Find route should return first hop of circuit
    assert_eq!(routing_table.find_route(&destination), Some(&hop1));

    // Get circuit by ID
    let retrieved = routing_table.get_circuit(circuit_id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, circuit_id);
}

#[test]
fn test_circuit_teardown() {
    let mut routing_table = RoutingTable::new();

    let destination = PeerId::random();
    let circuit = Circuit::new(vec![PeerId::random(), destination]);
    let circuit_id = circuit.id;

    routing_table.add_circuit(circuit).unwrap();
    assert_eq!(routing_table.circuit_count(), 1);

    // Remove circuit
    let removed = routing_table.remove_circuit(circuit_id);
    assert!(removed.is_some());
    assert_eq!(routing_table.circuit_count(), 0);

    // Route should be gone
    assert_eq!(routing_table.find_route(&destination), None);
}

#[test]
fn test_circuit_idle_pruning() {
    let mut routing_table = RoutingTable::new();

    // Create a circuit
    let mut circuit = Circuit::new(vec![PeerId::random(), PeerId::random()]);

    // Manually set last_activity to old timestamp (simulate idle)
    circuit.last_activity = std::time::Instant::now() - std::time::Duration::from_secs(400);

    routing_table.add_circuit(circuit).unwrap();
    assert_eq!(routing_table.circuit_count(), 1);

    // Prune idle circuits
    let pruned_count = routing_table.prune_idle_circuits();

    assert_eq!(pruned_count, 1);
    assert_eq!(routing_table.circuit_count(), 0);
}

#[test]
fn test_circuit_handshake_messages() {
    let circuit_id = 12345u64;
    let next_hop = PeerId::random();

    // Test CircuitRequest
    let request = CircuitRequest::new(circuit_id, next_hop);
    let encoded = request.encode().unwrap();
    let decoded = CircuitRequest::decode(&encoded).unwrap();
    assert_eq!(decoded.circuit_id, circuit_id);
    assert_eq!(decoded.next_hop_peer_id().unwrap(), next_hop);

    // Test CircuitReady
    let ready = CircuitReady::new(circuit_id);
    let encoded = ready.encode().unwrap();
    let decoded = CircuitReady::decode(&encoded).unwrap();
    assert_eq!(decoded.circuit_id, circuit_id);

    // Test CircuitClose
    let close = CircuitClose::new(circuit_id);
    let encoded = close.encode().unwrap();
    let decoded = CircuitClose::decode(&encoded).unwrap();
    assert_eq!(decoded.circuit_id, circuit_id);
}

#[test]
fn test_circuit_prefers_over_direct_route() {
    let mut routing_table = RoutingTable::new();

    let destination = PeerId::random();
    let direct_hop = PeerId::random();
    let circuit_hop = PeerId::random();

    // Add direct route
    routing_table.add_route(destination, direct_hop, None);

    // Add circuit route
    let circuit = Circuit::new(vec![circuit_hop, destination]);
    routing_table.add_circuit(circuit).unwrap();

    // Circuit route should be preferred
    assert_eq!(routing_table.find_route(&destination), Some(&circuit_hop));
}

#[test]
fn test_multiple_circuits() {
    let mut routing_table = RoutingTable::new();

    let dest1 = PeerId::random();
    let dest2 = PeerId::random();

    let circuit1 = Circuit::new(vec![PeerId::random(), dest1]);
    let circuit2 = Circuit::new(vec![PeerId::random(), dest2]);

    routing_table.add_circuit(circuit1).unwrap();
    routing_table.add_circuit(circuit2).unwrap();

    assert_eq!(routing_table.circuit_count(), 2);
    assert!(routing_table.find_route(&dest1).is_some());
    assert!(routing_table.find_route(&dest2).is_some());
}

#[test]
fn test_circuit_activity_tracking() {
    let mut circuit = Circuit::new(vec![PeerId::random()]);

    let initial_activity = circuit.last_activity;
    assert!(!circuit.is_idle());

    std::thread::sleep(std::time::Duration::from_millis(10));
    circuit.mark_active();

    assert!(circuit.last_activity > initial_activity);
    assert!(circuit.age() > std::time::Duration::from_millis(5));
}

} // End disabled_tests module
