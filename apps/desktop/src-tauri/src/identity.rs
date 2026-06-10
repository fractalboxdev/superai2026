//! First-run identity (spec 10 §4 step 2): keypair in the Keychain, then the
//! binary's own control plane (`ctl seed` / `ctl mint`) does the authority
//! work — the shell never mints tokens itself.

use std::process::Command;

use serde::Serialize;

use crate::settings::{AppSettings, Role};
use crate::{keychain, sidecar};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub principal: String,
    pub keychain_service: String,
    pub created: bool,
}

/// Seeded demo principals offered by the wizard (spec 07 §3 control plane).
/// Mirrors `crates/sync/src/scenario.rs` `principals()` — keep the lists in sync.
pub fn known_principals() -> Vec<String> {
    vec!["cfo".into(), "agent:cto/1".into(), "agent:eng/1".into()]
}

pub fn ensure(principal: &str, role: Role, settings: &AppSettings) -> anyhow::Result<IdentityInfo> {
    let created = !keychain::has_key(principal);
    if created {
        let key = keychain::generate_key()?;
        keychain::store_key(principal, &key)?;
    }

    let bin = sidecar::resolve()
        .ok_or_else(|| anyhow::anyhow!("sync binary not found in the app bundle"))?;

    let run_ctl = |args: &[&str]| -> anyhow::Result<()> {
        let mut cmd = Command::new(&bin);
        cmd.arg("ctl").args(args);
        // Same env as the supervised child, so `ctl` and the daemon agree
        // on the brain home (and inference mode).
        cmd.envs(settings.sidecar_envs());
        crate::util::run_checked(&mut cmd, &format!("sync ctl {}", args.join(" ")))?;
        Ok(())
    };

    if role == Role::Host {
        run_ctl(&["seed"])?;
    }
    // (Re)issue the principal's initial capability token. Unknown principals
    // fail loudly here, which the wizard surfaces as-is.
    run_ctl(&["mint", "--principal", principal])?;

    Ok(IdentityInfo {
        principal: principal.to_string(),
        keychain_service: keychain::SERVICE.to_string(),
        created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Drift guard: must match `IdentityInfo` in apps/desktop/src/ipc.ts.
    #[test]
    fn identity_info_keys_mirror_ipc_ts() {
        let info = IdentityInfo {
            principal: "cfo".into(),
            keychain_service: crate::keychain::SERVICE.into(),
            created: true,
        };
        let v = serde_json::to_value(info).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["created", "keychainService", "principal"]);
    }
}
