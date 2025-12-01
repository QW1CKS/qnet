#!/bin/bash
# =============================================================================
# QNet Super Peer Deployment Script
# =============================================================================
# 
# This script automates the full deployment of a QNet super peer on Ubuntu.
# Run this on a fresh Ubuntu 22.04+ droplet via SSH.
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/QW1CKS/qnet/main/scripts/deploy-super-peer.sh | bash
#
# Or download and run:
#   wget https://raw.githubusercontent.com/QW1CKS/qnet/main/scripts/deploy-super-peer.sh
#   chmod +x deploy-super-peer.sh
#   ./deploy-super-peer.sh
#
# =============================================================================

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
QNET_USER="qnet"
QNET_DIR="/opt/qnet"
QNET_REPO="https://github.com/QW1CKS/qnet.git"
QNET_BRANCH="main"
STEALTH_MODE="super"
SOCKS_PORT="1088"
STATUS_PORT="8088"
LIBP2P_PORT="4001"

# =============================================================================
# Helper Functions
# =============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_banner() {
    echo -e "${GREEN}"
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║           QNet Super Peer Deployment Script                   ║"
    echo "║                                                               ║"
    echo "║  This will install and configure a QNet super peer node       ║"
    echo "║  Mode: Bootstrap + Relay + Exit                               ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

get_public_ip() {
    curl -s ifconfig.me || curl -s icanhazip.com || echo "unknown"
}

# =============================================================================
# Step 1: System Update & Dependencies
# =============================================================================

install_dependencies() {
    log_info "Step 1/7: Updating system and installing dependencies..."
    
    # Update package lists
    apt-get update -qq
    
    # Upgrade existing packages
    DEBIAN_FRONTEND=noninteractive apt-get upgrade -y -qq
    
    # Install build dependencies
    apt-get install -y -qq \
        build-essential \
        pkg-config \
        libssl-dev \
        curl \
        git \
        htop \
        jq \
        ufw \
        fail2ban
    
    log_success "System dependencies installed"
}

# =============================================================================
# Step 2: Install Rust
# =============================================================================

install_rust() {
    log_info "Step 2/7: Installing Rust toolchain..."
    
    # Check if Rust is already installed
    if command -v rustc &> /dev/null; then
        log_warn "Rust already installed: $(rustc --version)"
        return
    fi
    
    # Install Rust via rustup (non-interactive)
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    
    # Source cargo environment
    source "$HOME/.cargo/env"
    
    log_success "Rust installed: $(rustc --version)"
}

# =============================================================================
# Step 3: Create QNet User
# =============================================================================

create_qnet_user() {
    log_info "Step 3/7: Creating qnet system user..."
    
    # Create user if doesn't exist
    if id "$QNET_USER" &>/dev/null; then
        log_warn "User '$QNET_USER' already exists"
    else
        useradd --system --create-home --shell /bin/bash "$QNET_USER"
        log_success "User '$QNET_USER' created"
    fi
    
    # Create opt directory
    mkdir -p "$QNET_DIR"
    chown -R "$QNET_USER:$QNET_USER" "$QNET_DIR"
}

# =============================================================================
# Step 4: Clone Repository
# =============================================================================

clone_repository() {
    log_info "Step 4/7: Cloning QNet repository..."
    
    # Remove existing directory if present
    if [[ -d "$QNET_DIR/.git" ]]; then
        log_warn "Repository already exists, pulling latest..."
        cd "$QNET_DIR"
        git fetch origin
        git checkout "$QNET_BRANCH"
        git pull origin "$QNET_BRANCH"
    else
        # Fresh clone - clone as root first, then chown
        rm -rf "$QNET_DIR"
        git clone "$QNET_REPO" "$QNET_DIR"
        cd "$QNET_DIR"
        git checkout "$QNET_BRANCH"
    fi
    
    # Ensure qnet user owns the directory
    chown -R "$QNET_USER:$QNET_USER" "$QNET_DIR"
    
    log_success "Repository cloned to $QNET_DIR"
}

# =============================================================================
# Step 5: Build Release Binary
# =============================================================================

build_binary() {
    log_info "Step 5/7: Building QNet binary (this may take 5-10 minutes)..."
    
    cd "$QNET_DIR"
    
    # Ensure cargo is available (installed for root)
    source "$HOME/.cargo/env" 2>/dev/null || true
    
    # Build release binary as root (faster, avoids Rust install for qnet user)
    cargo build --release -p stealth-browser
    
    # Ensure qnet user owns the built artifacts
    chown -R "$QNET_USER:$QNET_USER" "$QNET_DIR"
    
    # Verify binary exists
    if [[ -f "$QNET_DIR/target/release/stealth-browser" ]]; then
        log_success "Binary built: $QNET_DIR/target/release/stealth-browser"
        ls -lh "$QNET_DIR/target/release/stealth-browser"
    else
        log_error "Build failed - binary not found"
        exit 1
    fi
}

# =============================================================================
# Step 6: Generate Persistent Keypair
# =============================================================================

generate_persistent_keypair() {
    log_info "Step 6/8: Generating persistent keypair for stable peer ID..."
    
    KEYPAIR_DIR="$QNET_DIR/data"
    KEYPAIR_PATH="$KEYPAIR_DIR/keypair.pb"
    
    # Create data directory
    mkdir -p "$KEYPAIR_DIR"
    chown -R "$QNET_USER:$QNET_USER" "$KEYPAIR_DIR"
    
    # Check if keypair already exists
    if [[ -f "$KEYPAIR_PATH" ]]; then
        log_warn "Keypair already exists at $KEYPAIR_PATH"
        log_warn "To regenerate, delete the file and re-run deployment"
    else
        # Generate new keypair
        cd "$QNET_DIR"
        source "$HOME/.cargo/env" 2>/dev/null || true
        
        "$QNET_DIR/target/release/stealth-browser" --generate-keypair "$KEYPAIR_PATH"
        
        chown "$QNET_USER:$QNET_USER" "$KEYPAIR_PATH"
        chmod 600 "$KEYPAIR_PATH"
        
        log_success "Persistent keypair generated at $KEYPAIR_PATH"
    fi
    
    # Extract and display peer ID for operator configuration
    PEER_ID=$("$QNET_DIR/target/release/stealth-browser" --generate-keypair /dev/null 2>&1 | grep -oP '12D3KooW[A-Za-z0-9]+' || echo "")
    if [[ -z "$PEER_ID" ]]; then
        log_warn "Could not extract peer ID - check logs after service starts"
    fi
}

# =============================================================================
# Step 7: Create Systemd Service
# =============================================================================

create_systemd_service() {
    log_info "Step 7/8: Creating systemd service..."
    
    PUBLIC_IP=$(get_public_ip)
    KEYPAIR_PATH="$QNET_DIR/data/keypair.pb"
    
    cat > /etc/systemd/system/qnet-super.service << EOF
[Unit]
Description=QNet Super Peer (Bootstrap + Relay + Exit)
Documentation=https://github.com/QW1CKS/qnet
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$QNET_USER
Group=$QNET_USER
WorkingDirectory=$QNET_DIR

# Environment configuration
Environment="STEALTH_MODE=$STEALTH_MODE"
Environment="STEALTH_SOCKS_PORT=$SOCKS_PORT"
Environment="STEALTH_STATUS_PORT=$STATUS_PORT"
Environment="QNET_STATUS_BIND=0.0.0.0"
Environment="QNET_SOCKS_BIND=0.0.0.0"
Environment="QNET_KEYPAIR_PATH=$KEYPAIR_PATH"
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

# The binary
ExecStart=$QNET_DIR/target/release/stealth-browser

# Restart policy
Restart=always
RestartSec=10
StartLimitInterval=60
StartLimitBurst=3

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=$QNET_DIR/logs $QNET_DIR/data

# Resource limits
LimitNOFILE=65535
MemoryMax=512M

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=qnet-super

[Install]
WantedBy=multi-user.target
EOF

    # Create logs directory
    mkdir -p "$QNET_DIR/logs"
    chown -R "$QNET_USER:$QNET_USER" "$QNET_DIR/logs"
    
    # Reload systemd
    systemctl daemon-reload
    
    log_success "Systemd service created: qnet-super.service"
}

# =============================================================================
# Step 8: Configure Firewall
# =============================================================================

configure_firewall() {
    log_info "Step 8/8: Configuring firewall..."
    
    # Enable UFW if not already
    ufw --force enable
    
    # Allow SSH (important - don't lock yourself out!)
    ufw allow 22/tcp comment 'SSH'
    
    # Allow QNet ports
    ufw allow $STATUS_PORT/tcp comment 'QNet Status API'
    ufw allow $SOCKS_PORT/tcp comment 'QNet SOCKS5 Proxy'
    ufw allow $LIBP2P_PORT/tcp comment 'QNet libp2p'
    ufw allow $LIBP2P_PORT/udp comment 'QNet libp2p QUIC'
    
    # Show status
    ufw status verbose
    
    log_success "Firewall configured"
}

# =============================================================================
# Start Service
# =============================================================================

start_service() {
    log_info "Starting QNet super peer service..."
    
    systemctl enable qnet-super
    systemctl start qnet-super
    
    # Wait a moment for startup
    sleep 3
    
    # Check status
    if systemctl is-active --quiet qnet-super; then
        log_success "QNet super peer is running!"
    else
        log_error "Service failed to start. Check logs with: journalctl -u qnet-super -f"
        systemctl status qnet-super
        exit 1
    fi
}

# =============================================================================
# Print Summary
# =============================================================================

print_summary() {
    PUBLIC_IP=$(get_public_ip)
    KEYPAIR_PATH="$QNET_DIR/data/keypair.pb"
    
    # Get peer ID from the keypair file by running the binary briefly
    PEER_ID=""
    if [[ -f "$KEYPAIR_PATH" ]]; then
        # Start the service briefly to capture peer ID from logs
        PEER_ID=$(journalctl -u qnet-super --since "1 minute ago" 2>/dev/null | grep -oP '12D3KooW[A-Za-z0-9]+' | head -1 || echo "")
    fi
    
    echo ""
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║              🎉 Deployment Complete! 🎉                       ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${BLUE}Server Information:${NC}"
    echo "  Public IP:     $PUBLIC_IP"
    echo "  Mode:          $STEALTH_MODE"
    if [[ -n "$PEER_ID" ]]; then
        echo "  Peer ID:       $PEER_ID"
    fi
    echo ""
    echo -e "${BLUE}Endpoints:${NC}"
    echo "  Status API:    http://$PUBLIC_IP:$STATUS_PORT/status"
    echo "  Directory:     http://$PUBLIC_IP:$STATUS_PORT/api/relays/by-country"
    echo "  SOCKS5 Proxy:  $PUBLIC_IP:$SOCKS_PORT"
    if [[ -n "$PEER_ID" ]]; then
        echo "  libp2p:        /ip4/$PUBLIC_IP/tcp/$LIBP2P_PORT/p2p/$PEER_ID"
    else
        echo "  libp2p:        /ip4/$PUBLIC_IP/tcp/$LIBP2P_PORT/p2p/<PEER_ID>"
    fi
    echo ""
    echo -e "${BLUE}Persistent Identity:${NC}"
    echo "  Keypair file:  $KEYPAIR_PATH"
    if [[ -n "$PEER_ID" ]]; then
        echo "  Peer ID:       $PEER_ID (stable across restarts)"
    else
        echo "  Get Peer ID:   journalctl -u qnet-super | grep 'local_peer_id'"
    fi
    echo ""
    echo -e "${BLUE}Useful Commands:${NC}"
    echo "  View logs:     journalctl -u qnet-super -f"
    echo "  Restart:       systemctl restart qnet-super"
    echo "  Stop:          systemctl stop qnet-super"
    echo "  Status:        systemctl status qnet-super"
    echo ""
    echo -e "${BLUE}Quick Test:${NC}"
    echo "  curl http://$PUBLIC_IP:$STATUS_PORT/status | jq"
    echo "  curl http://$PUBLIC_IP:$STATUS_PORT/ping"
    echo ""
    echo -e "${YELLOW}⚠️  Exit Node Warning:${NC}"
    echo "  This node will make web requests for other users."
    echo "  Your IP ($PUBLIC_IP) will be visible to destination websites."
    echo "  Ensure compliance with local laws and DigitalOcean ToS."
    echo ""
    if [[ -n "$PEER_ID" ]]; then
        echo -e "${GREEN}Next Steps:${NC}"
        echo "  Update hardcoded_operator_nodes() in crates/core-mesh/src/discovery.rs:"
        echo "     /ip4/$PUBLIC_IP/tcp/$LIBP2P_PORT/p2p/$PEER_ID"
        echo ""
    else
        echo -e "${GREEN}Next Steps:${NC}"
        echo "  1. Get the stable peer ID from logs:"
        echo "     journalctl -u qnet-super | grep 'local_peer_id'"
        echo ""
        echo "  2. Update hardcoded_operator_nodes() in your local code with:"
        echo "     /ip4/$PUBLIC_IP/tcp/$LIBP2P_PORT/p2p/<PEER_ID>"
        echo ""
    fi
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    print_banner
    check_root
    
    log_info "Starting deployment on $(hostname)..."
    log_info "Public IP: $(get_public_ip)"
    echo ""
    
    install_dependencies
    install_rust
    create_qnet_user
    clone_repository
    build_binary
    generate_persistent_keypair
    create_systemd_service
    configure_firewall
    start_service
    print_summary
}

# Run main function
main "$@"
