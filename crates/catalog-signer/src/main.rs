use std::{fs, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ring::signature::KeyPair;

/// Env var containing the hex-encoded 32-byte Ed25519 private seed
const ENV_PRIVKEY: &str = "CATALOG_PRIVKEY";

#[derive(Parser, Debug)]
#[command(name = "catalog-signer", version, about = "Sign and verify QNet catalogs")] 
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a catalog from YAML templates and sign it
    Sign(SignArgs),
    /// Verify a catalog against a detached signature and a pinned public key
    Verify(VerifyArgs),
    /// Derive and print the Ed25519 public key (hex) from CATALOG_PRIVKEY seed
    Pubkey,
}

#[derive(Parser, Debug)]
struct SignArgs {
    /// Path to decoys.yml
    #[arg(long)]
    decoys: PathBuf,
    /// Path to catalog.meta.yml
    #[arg(long)]
    meta: PathBuf,
    /// Output catalog.json path
    #[arg(long)]
    out: PathBuf,
    /// Output detached signature path (hex). If omitted and --inline is set, signature is embedded as signature_hex
    #[arg(long)]
    sig: Option<PathBuf>,
    /// Embed signature into catalog.json as signature_hex instead of writing a detached .sig
    #[arg(long, default_value_t = false)]
    inline: bool,
    /// Override expires window relative to now, e.g., 7d, 24h. If not set, default 7d.
    #[arg(long)]
    expires: Option<String>,
    /// Set generated_at time explicitly (RFC3339). If not set, uses now.
    #[arg(long)]
    now: Option<String>,
}

#[derive(Parser, Debug)]
struct VerifyArgs {
    /// Path to catalog.json
    #[arg(long)]
    catalog: PathBuf,
    /// Path to detached signature file (hex) if catalog doesn't inline signature_hex
    #[arg(long)]
    sig: Option<PathBuf>,
    /// Pinned publisher public key file (hex, 32 bytes)
    #[arg(long, required = true)]
    pubkey_file: PathBuf,
}

#[derive(Debug, Deserialize)]
struct DecoyTemplates {
    schema_version: u64,
    publisher_id: String,
    entries: Vec<CatalogEntry>,
}

#[derive(Debug, Deserialize)]
struct MetaTemplates {
    schema_version: u64,
    catalog_version: u64,
    publisher_id: String,
    #[serde(default)]
    generated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    expires_at: Option<DateTime<Utc>>,
    update_urls: Vec<String>,
    #[serde(default)]
    seed_fallback_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogEntry {
    id: String,
    host: String,
    ports: Vec<u16>,
    protocols: Vec<String>,
    alpn: Vec<String>,
    #[serde(default)]
    region: Vec<String>,
    #[serde(default)]
    weight: Option<u64>,
    #[serde(default)]
    health_path: Option<String>,
    #[serde(default)]
    tls_profile: Option<String>,
    #[serde(default)]
    quic_hints: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogInner {
    schema_version: u64,
    catalog_version: u64,
    generated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    publisher_id: String,
    update_urls: Vec<String>,
    #[serde(default)]
    seed_fallback_urls: Vec<String>,
    entries: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogOuter {
    #[serde(flatten)]
    inner: CatalogInner,
    /// Optional inline hex signature. If absent, use detached .sig file.
    #[serde(default)]
    signature_hex: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Sign(args) => sign_cmd(args),
        Commands::Verify(args) => verify_cmd(args),
        Commands::Pubkey => pubkey_cmd(),
    }
}

fn sign_cmd(args: SignArgs) -> Result<()> {
    // Load templates
    let decoys: DecoyTemplates = serde_yaml::from_str(&fs::read_to_string(&args.decoys).with_context(|| format!("read {:?}", &args.decoys))?)?;
    let meta: MetaTemplates = serde_yaml::from_str(&fs::read_to_string(&args.meta).with_context(|| format!("read {:?}", &args.meta))?)?;

    // Basic consistency checks
    if decoys.schema_version != meta.schema_version {
        return Err(anyhow!("schema_version mismatch between decoys and meta"));
    }
    if decoys.publisher_id != meta.publisher_id {
        return Err(anyhow!("publisher_id mismatch between decoys and meta"));
    }

    let now: DateTime<Utc> = if let Some(s) = &args.now {
        s.parse().context("parse --now RFC3339")?
    } else {
        Utc::now()
    };

    let expires_at = if let Some(exp) = &args.expires {
        // naive parser: Nd or Nh
        parse_expiry(&now, exp)?
    } else if let Some(e) = meta.expires_at {
        e
    } else {
        now + chrono::Duration::days(7)
    };

    let generated_at = meta.generated_at.unwrap_or(now);

    let inner = CatalogInner {
        schema_version: meta.schema_version,
        catalog_version: meta.catalog_version,
        generated_at,
        expires_at,
        publisher_id: meta.publisher_id,
        update_urls: meta.update_urls,
        seed_fallback_urls: meta.seed_fallback_urls,
        entries: decoys.entries,
    };

    // Encode DET-CBOR of inner and sign
    let det = core_cbor::to_det_cbor(&inner).context("encode DET-CBOR")?;

    // Fetch private key seed from env
    let seed_hex = std::env::var(ENV_PRIVKEY).context(format!("missing env {} (hex 32 bytes)", ENV_PRIVKEY))?;
    let seed = hex::decode(seed_hex.trim()).context("hex decode seed")?;
    if seed.len() != 32 {
        return Err(anyhow!("CATALOG_PRIVKEY must be 32 bytes hex"));
    }
    let mut seed32 = [0u8; 32];
    seed32.copy_from_slice(&seed);

    let sig = core_crypto::ed25519::sign(&seed32, &det);
    let sig_hex = hex::encode(sig);

    // Prepare outputs
    if args.inline {
        let outer = CatalogOuter { inner, signature_hex: Some(sig_hex) };
        write_json(&args.out, &outer)?;
    } else {
       // Detached: write inner as plain JSON (without signature), and .sig file
        write_json(&args.out, &inner)?;
        let sig_path = args.sig.ok_or_else(|| anyhow!("--sig is required when not using --inline"))?;
        fs::create_dir_all(sig_path.parent().unwrap_or_else(|| std::path::Path::new(".")))?;
        fs::write(sig_path, sig_hex.as_bytes()).context("write .sig")?;
    }

    Ok(())
}

fn verify_cmd(args: VerifyArgs) -> Result<()> {
    // Load catalog JSON (could be outer with signature_hex or inner-only)
    let text = fs::read_to_string(&args.catalog).with_context(|| format!("read {:?}", &args.catalog))?;

    // Try outer first
    let outer: Result<CatalogOuter, _> = serde_json::from_str(&text);
    let (inner, sig_hex_opt): (CatalogInner, Option<String>) = match outer {
        Ok(outer) => (outer.inner, outer.signature_hex),
        Err(_) => {
            // Try inner
            let inner: CatalogInner = serde_json::from_str(&text).context("parse catalog as inner JSON")?;
            (inner, None)
        }
    };

    let sig_hex = match sig_hex_opt.or_else(|| {
        args.sig.as_ref().and_then(|p| fs::read_to_string(p).ok())
    }) {
        Some(s) => s.trim().to_string(),
        None => return Err(anyhow!("no signature found: neither signature_hex inline nor --sig provided")),
    };

    let sig = hex::decode(&sig_hex).context("hex decode sig")?;

    // Encode DET-CBOR of inner for verification
    let det = core_cbor::to_det_cbor(&inner).context("encode DET-CBOR")?;

    // Load pubkey; allow comment lines starting with '#'
    let raw = fs::read_to_string(&args.pubkey_file).context("read pubkey file")?;
    let pk_hex: String = raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect();
    let pk = hex::decode(pk_hex.trim()).context("hex decode pubkey")?;

    core_crypto::ed25519::verify(&pk, &det, &sig).map_err(|_| anyhow!("signature verify failed"))?;

    // Freshness check
    if inner.expires_at <= Utc::now() {
        eprintln!("warning: catalog is expired at {}", inner.expires_at);
    }

    println!("verify ok: schema_version={}, catalog_version={}, entries={}", inner.schema_version, inner.catalog_version, inner.entries.len());
    Ok(())
}

fn pubkey_cmd() -> Result<()> {
    let seed_hex = std::env::var(ENV_PRIVKEY).context(format!("missing env {} (hex 32 bytes)", ENV_PRIVKEY))?;
    let seed = hex::decode(seed_hex.trim()).context("hex decode seed")?;
    if seed.len() != 32 { return Err(anyhow!("CATALOG_PRIVKEY must be 32 bytes hex")); }
    let kp = ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed).expect("ed25519 seed");
    let pk = kp.public_key().as_ref();
    println!("{}", hex::encode(pk));
    Ok(())
}

fn write_json<P: Into<PathBuf>, T: Serialize>(path: P, value: &T) -> Result<()> {
    let path: PathBuf = path.into();
    if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
    let data = serde_json::to_vec_pretty(value)?;
    fs::write(&path, data).with_context(|| format!("write {:?}", path))?;
    Ok(())
}

fn parse_expiry(now: &DateTime<Utc>, s: &str) -> Result<DateTime<Utc>> {
    // Supports Nd or Nh (e.g., 7d, 24h)
    if let Some(num) = s.strip_suffix('d') {
        let n: i64 = num.parse().context("parse days")?;
        return Ok(*now + chrono::Duration::days(n));
    }
    if let Some(num) = s.strip_suffix('h') {
        let n: i64 = num.parse().context("parse hours")?;
        return Ok(*now + chrono::Duration::hours(n));
    }
    Err(anyhow!("unsupported expires format: use Nd or Nh, e.g., 7d"))
}
