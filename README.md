# QNet: The Invisible Overlay Network

<div align="center">
  <img src="logo.png" alt="QNet Logo" width="400">
  <p><strong>Decentralized. Censorship-Resistant. Unblockable.</strong></p>
</div>

---

> [!CAUTION]
> Most of the code has been implemented using agentic AI. This is just a side-project that I wanted to experiment with Copilot. This project was done purely for fun and learning. I will be removing the AI-generated code  and implement it manually in the future if I ever plan to make it production-ready. I know how frustrating it is to see AI slop in production code these days, and I very much understand the sentiment from a security perspective.
>
> If I ever intend to make this production-ready, I will make sure to undergo a professional security audit for this project.
>
> At the current moment, I make the AI follow strict [security guardrails](qnet-spec/memory/ai-guardrail.md) to ensure that the code is secure and follows best practices.
>
> Use at your own risk.


## 🧐 What is QNet?

QNet is a **decentralized overlay network** that allows you to access the free internet from anywhere.

It works by **disguising your traffic** as normal connections to popular sites (like Microsoft, Google, or Cloudflare). To an ISP or censor, you are just browsing a harmless website. In reality, you are routing encrypted traffic through a global mesh of peers to reach your true destination.

## 🚀 How It Works

Unlike a VPN (centralized) or Tor (slow), QNet uses a **Browser Extension + Helper** model:

1.  **The Extension**: You install a simple button in your browser.
2.  **The Helper**: A small background service runs on your computer.
3.  **The Mesh**: Your Helper joins the QNet P2P mesh.

When you browse:
- **You** want `amazon.com`.
- **QNet** connects to `microsoft.com` (a decoy node).
- **ISP** sees `HTTPS -> microsoft.com`.
- **You** get `amazon.com` content.

## ✨ Key Features

- **🎭 Perfect Disguise**: Uses **HTX (Hypertext Transport Extension)** to clone TLS fingerprints of popular sites. Indistinguishable from real traffic.
- **🕸️ Fully Decentralized**: No central servers to block. Every user helps the network (P2P).
- **⚡ Performance Choice**: Choose **Fast Mode** (1-hop) for speed or **Privacy Mode** (3-hop) for anonymity.
- **🔒 Secure**: Built with Rust, ChaCha20-Poly1305, and Noise XK encryption.

## 🛠️ Quick Start (Developers)

QNet is currently in **Active Development**. You can build and run the components today.

### Prerequisites
- **Rust 1.70+**
- **Windows** (Primary dev environment) or Linux/macOS

### Build & Run
```powershell
# 1. Clone the repo
git clone https://github.com/QW1CKS/qnet.git
cd qnet

# 2. Build everything
cargo build --workspace

# 3. Run the Helper (Stealth Browser)
cargo run -p stealth-browser
```

### Smoke Test (Masked Connection)
Verify that QNet can disguise a connection:
```powershell
# Connect to wikipedia.org disguised as a decoy
pwsh ./scripts/test-masked-connect.ps1 -Target www.wikipedia.org
```

## 📚 Documentation

- **[Unified Task List](qnet-spec/specs/001-qnet/tasks.md)**: The master plan.
- **[Protocol Spec](qnet-spec/specs/001-qnet/spec.md)**: How it works under the hood.
- **[Roadmap](qnet-spec/specs/001-qnet/plan.md)**: Where we are going.

## 🤝 Contributing

We are building the future of internet freedom.
1.  Check **[tasks.md](qnet-spec/specs/001-qnet/tasks.md)** for open items.
2.  Pick a task from **Phase 2** or **Phase 3**.
3.  Submit a PR!

---
*Licensed under MIT.*