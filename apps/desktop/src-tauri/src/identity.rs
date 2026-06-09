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
        if let Some(home) = settings.brain_home_expanded() {
            cmd.env("CONTEXTFUL_HOME", home);
        }
        let out = cmd.output()?;
        anyhow::ensure!(
            out.status.success(),
            "sync ctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
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
        keychain_service: keychain::service().to_string(),
        created,
    })
}
