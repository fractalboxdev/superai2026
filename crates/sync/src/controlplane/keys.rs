//! Resource-root keystore (spec 03 §1, spec 07 §3).
//!
//! Each resource root owns an ed25519 keypair. The private half lives here,
//! under `control/keys/<root>.key` (0600) — never in the registry, which only
//! carries the public half for verification.

use std::path::PathBuf;

use anyhow::{Context, Result};
use biscuit_auth::{Algorithm, KeyPair, PrivateKey, PublicKey};

use crate::config::Config;
use crate::controlplane::principal_key;

fn keys_dir(config: &Config) -> PathBuf {
    config.control_dir().join("keys")
}

fn key_path(config: &Config, root_id: &str) -> PathBuf {
    keys_dir(config).join(format!("{}.key", principal_key(root_id)))
}

/// Load a root's keypair, generating + persisting one if absent.
pub fn ensure_root_key(config: &Config, root_id: &str) -> Result<KeyPair> {
    if let Some(kp) = load_root_key(config, root_id)? {
        return Ok(kp);
    }
    let kp = KeyPair::new();
    std::fs::create_dir_all(keys_dir(config))?;
    let path = key_path(config, root_id);
    // create_new + mode(0600): the key must never exist world-readable (a
    // write-then-chmod leaves a readable window) and a concurrent seed must
    // not clobber a key whose public half is already in the registry.
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    match opts.open(&path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(kp.private().to_bytes_hex().as_bytes())
                .context("writing root key")?;
            Ok(kp)
        }
        // lost the creation race — the winner's key is authoritative
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => load_root_key(config, root_id)?
            .ok_or_else(|| anyhow::anyhow!("root key {} vanished after creation race", root_id)),
        Err(e) => Err(e).context("creating root key"),
    }
}

/// Load a root's keypair if its private key is in the keystore.
pub fn load_root_key(config: &Config, root_id: &str) -> Result<Option<KeyPair>> {
    let path = key_path(config, root_id);
    let hex_str = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("reading root key"),
    };
    let pk = PrivateKey::from_bytes_hex(hex_str.trim(), Algorithm::Ed25519)
        .map_err(|e| anyhow::anyhow!("bad root key {}: {e}", path.display()))?;
    Ok(Some(KeyPair::from(&pk)))
}

/// Parse a registry-carried public key (hex).
pub fn public_key_from_hex(hex_str: &str) -> Result<PublicKey> {
    PublicKey::from_bytes_hex(hex_str.trim(), Algorithm::Ed25519)
        .map_err(|e| anyhow::anyhow!("bad public key: {e}"))
}
