# QNet Quick Start Guide

Get up and running with QNet in 5 minutes!

## 📦 Installation

### Windows

**Option 1: Pre-built Binary (Recommended)**
```powershell
# Download latest release
Invoke-WebRequest -Uri "https://github.com/QW1CKS/qnet/releases/latest/download/stealth-browser-windows.zip" -OutFile "qnet.zip"

# Extract
Expand-Archive -Path "qnet.zip" -DestinationPath "C:\Program Files\QNet"

# Add to PATH
$env:Path += ";C:\Program Files\QNet"
```

**Option 2: Build from Source**
```powershell
# Clone repository
git clone https://github.com/QW1CKS/qnet.git
cd qnet

# Build release binary
cargo build -p stealth-browser --release

# Binary location: .\target\release\stealth-browser.exe
```

### Linux / macOS

**Option 1: Pre-built Binary**
```bash
# Download latest release
wget https://github.com/QW1CKS/qnet/releases/latest/download/stealth-browser-linux.tar.gz

# Extract
tar -xzf stealth-browser-linux.tar.gz -C /usr/local/bin/

# Make executable
chmod +x /usr/local/bin/stealth-browser
```

**Option 2: Build from Source**
```bash
# Clone repository
git clone https://github.com/QW1CKS/qnet.git
cd qnet

# Build release binary
cargo build -p stealth-browser --release

# Binary location: ./target/release/stealth-browser
```

## 🚀 Starting the Helper

### Basic Usage

**Windows:**
```powershell
# Start Helper with default settings
.\stealth-browser.exe

# Run in background
Start-Process -NoNewWindow -FilePath ".\stealth-browser.exe"
```

**Linux/macOS:**
```bash
# Start Helper
./stealth-browser

# Run in background
./stealth-browser > helper.log 2>&1 &
```

### Configuration Options

Set environment variables before starting:

```powershell
# Windows
$env:QNET_STATUS_BIND = "127.0.0.1:8088"  # Status API port
$env:RUST_LOG = "info"                     # Log level

# Linux/macOS
export QNET_STATUS_BIND="127.0.0.1:8088"
export RUST_LOG="info"
```

## ✅ Verify Installation

### Check Helper Status

**HTTP API:**
```powershell
# Windows
Invoke-WebRequest -Uri "http://127.0.0.1:8088/status" | ConvertFrom-Json

# Linux/macOS
curl http://127.0.0.1:8088/status | jq
```

**Expected Response:**
```json
{
  "mode": "socks5",
  "state": "running",
  "decoy_count": 5,
  "peers_online": 3,
  "nat_status": "unknown"
}
```

### Test SOCKS5 Proxy

```bash
# Using curl
curl --socks5 127.0.0.1:1088 https://example.com

# Using wget
wget -e use_proxy=yes -e http_proxy=socks5://127.0.0.1:1088 https://example.com
```

## 🌐 Browser Configuration

### Firefox

1. Open Settings → Network Settings → Settings
2. Select "Manual proxy configuration"
3. SOCKS Host: `127.0.0.1` Port: `1088`
4. Check "SOCKS v5"
5. Check "Proxy DNS when using SOCKS v5"

### Chrome/Edge

1. Install extension: [SwitchyOmega](https://chrome.google.com/webstore/detail/padekgcemlokbadohgkifijomclgjgif)
2. Create new profile: "QNet"
3. Protocol: SOCKS5
4. Server: `127.0.0.1` Port: `1088`
5. Click "Apply changes"

### System-Wide (Windows)

```powershell
# Set system proxy
netsh winhttp set proxy proxy-server="socks=127.0.0.1:1088" bypass-list="localhost;127.0.0.1"

# Reset proxy
netsh winhttp reset proxy
```

### System-Wide (Linux)

```bash
# Set environment variables (add to ~/.bashrc)
export http_proxy="socks5://127.0.0.1:1088"
export https_proxy="socks5://127.0.0.1:1088"
export all_proxy="socks5://127.0.0.1:1088"
```

## 🎯 First Connection Test

### Test Basic Connectivity

```powershell
# Windows - Test with PowerShell
$proxy = New-Object System.Net.WebProxy("socks5://127.0.0.1:1088")
$handler = New-Object System.Net.Http.HttpClientHandler
$handler.Proxy = $proxy
$client = New-Object System.Net.Http.HttpClient($handler)
$response = $client.GetAsync("https://example.com").Result
Write-Host $response.StatusCode
```

```bash
# Linux/macOS - Test with curl
curl --socks5 127.0.0.1:1088 https://example.com -I
```

### Check Peer Discovery

```powershell
# Windows
Invoke-WebRequest -Uri "http://127.0.0.1:8088/status" | ConvertFrom-Json | Select-Object peers_online

# Linux/macOS
curl -s http://127.0.0.1:8088/status | jq '.peers_online'
```

## 🔧 Troubleshooting

### Helper Won't Start

**Port Already in Use:**
```powershell
# Windows - Find process using port
netstat -ano | findstr :1088
taskkill /PID <process_id> /F

# Linux/macOS
lsof -i :1088
kill <PID>
```

**Permission Denied:**
```bash
# Linux - Allow binding to low ports
sudo setcap 'cap_net_bind_service=+ep' ./stealth-browser
```

### No Peers Discovered

1. Check firewall settings - ensure UDP ports are open
2. Verify internet connectivity
3. Check logs: `RUST_LOG=debug ./stealth-browser`
4. Wait 30-60 seconds for DHT bootstrap

### Connection Timeouts

1. Verify Helper is running: `http://127.0.0.1:8088/status`
2. Check SOCKS5 port: `1088` (default)
3. Ensure browser/app is configured correctly
4. Check Helper logs for errors

## 📖 Next Steps

- **[Browser Extension Guide](../qnet-spec/docs/extension.md)** - Install the UI
- **[Architecture Overview](ARCHITECTURE.md)** - How it works
- **[Troubleshooting](TROUBLESHOOTING.md)** - Common issues
- **[Contributing](CONTRIBUTING.md)** - Join development

## 🆘 Getting Help

- **GitHub Issues**: [Report bugs](https://github.com/QW1CKS/qnet/issues)
- **Discussions**: [Ask questions](https://github.com/QW1CKS/qnet/discussions)
- **Security**: See [SECURITY.md](../SECURITY.md)

---

**Welcome to QNet! You're now part of building the unblockable internet.** 🎉
