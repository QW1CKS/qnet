use std::time::Duration;

fn main() {
    let timeout = std::env::var("BOOTSTRAP_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(29);
    match htx::bootstrap::connect_seed_from_env(Duration::from_secs(timeout)) {
        Some(url) => {
            println!("bootstrap: ok -> {}", url);
        }
        None => {
            eprintln!("bootstrap: no healthy seed found (check STEALTH_BOOTSTRAP_* env)");
        }
    }
}
