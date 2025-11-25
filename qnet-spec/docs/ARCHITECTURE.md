# QNet System Architecture

## High-Level Components

The QNet system consists of three main components:

1.  **Browser Extension (`apps/extension`)**:
    - **Role**: User Interface & Proxy Controller.
    - **Tech**: WebExtensions API (JS/HTML/CSS).
    - **Communication**: Native Messaging to the Helper.

2.  **Helper Service (`apps/stealth-browser`)**:
    - **Role**: The core logic engine. Acts as a SOCKS5 server, P2P node, and HTX client.
    - **Tech**: Rust (Tokio, Hyper, Libp2p).
    - **Communication**:
        - **Input**: SOCKS5 from Browser, Native Messaging commands.
        - **Output**: Encrypted HTX traffic to the Mesh.

3.  **The Mesh (`crates/core-mesh`)**:
    - **Role**: The global network of peers.
    - **Tech**: Libp2p (Noise, Yamux, Kademlia DHT).

## Data Flow

### 1. User Request
User types `amazon.com` in the browser.

### 2. Extension Interception
The extension (via PAC script or Proxy API) redirects the request to the local SOCKS5 proxy: `127.0.0.1:1088`.

### 3. Helper Processing
The Helper receives the SOCKS5 request for `amazon.com`.
1.  **Catalog Lookup**: Checks the signed catalog for a valid **Decoy Node** (Entry Node).
2.  **Path Selection**: Determines the route (Direct to Decoy, or via Mesh Peers).
3.  **HTX Encapsulation**:
    - Generates a ClientHello matching the Decoy's fingerprint (e.g., Microsoft).
    - Establishes a TLS connection to the Decoy.
    - Performs an inner Noise XK handshake.

### 4. Mesh Routing
The encrypted packet travels through the mesh.
- **Entry Node**: Receives the HTX packet, decrypts the inner layer.
- **Relay Nodes**: Forward the packet based on the circuit ID.
- **Exit Node**: Makes the actual TCP connection to `amazon.com`.

### 5. Response
The response follows the reverse path, encrypted at each step, until it reaches the Helper, which unwraps it and sends it back to the browser via SOCKS5.

## Directory Structure

### `apps/` (User Facing)
- `stealth-browser/`: The Helper binary (Rust).
- `extension/`: The Browser Extension (JS).

### `crates/` (Core Logic)
- `htx/`: The masking transport layer.
- `core-crypto/`: Cryptographic primitives.
- `core-framing/`: Wire protocol definitions.
- `core-mesh/`: P2P networking logic.
- `catalog-signer/`: Tooling for the update system.

## Security Model

- **Trust**: We do NOT trust the ISP. We do NOT trust individual peers with metadata (onion routing).
- **Updates**: All updates (catalogs, binaries) must be signed by the developer keys.
- **Fingerprinting**: We assume the ISP performs Deep Packet Inspection (DPI) and matches TLS fingerprints against known browsers.
