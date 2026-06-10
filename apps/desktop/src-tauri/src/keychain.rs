//! Principal private key in the macOS Keychain (spec 10 §4 step 2).
//! Shells out to `/usr/bin/security` — no plaintext key ever touches disk.

use std::process::Command;

pub const SERVICE: &str = "work.contextful";

pub fn has_key(principal: &str) -> bool {
    Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", principal])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn store_key(principal: &str, secret_hex: &str) -> anyhow::Result<()> {
    crate::util::run_checked(
        Command::new("/usr/bin/security").args([
            "add-generic-password",
            "-U", // update if present
            "-s",
            SERVICE,
            "-a",
            principal,
            "-l",
            "Contextful principal key",
            "-w",
            secret_hex,
        ]),
        "keychain write",
    )?;
    Ok(())
}

/// 32 random bytes, hex-encoded (ed25519-seed-shaped; the binary's Biscuit
/// path is the eventual consumer — spec 03 "Future").
pub fn generate_key() -> anyhow::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom")?;
    let mut buf = [0u8; 32];
    f.read_exact(&mut buf)?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_key_is_32_bytes_hex() {
        let k = super::generate_key().unwrap();
        assert_eq!(k.len(), 64);
        assert!(k.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
