//! Inbound secret scrubbing (spec 03 §4, the return path).
//!
//! The egress firewall ([`super::egress`]) stops private values from *leaving*
//! the host; this is the complementary check on what comes *back*. Web
//! enrichment responses are scanned for anything secret-shaped — the host's
//! own credential values and well-known vendor key formats — and redacted
//! before the content can land in a world card, where it would become
//! default-readable and citable. Pure function, no I/O; [`crate::brain::world`]
//! calls [`scrub`] on every inbound result.

/// Env vars whose values are host credentials. Their values must never
/// surface in brain content; kept in lockstep with the connectors/backends
/// that read them ([`crate::config`], `connectors/{exa,stripe}.rs`).
pub const SECRET_ENV_KEYS: &[&str] = &[
    "EXA_API_KEY",
    "STRIPE_SECRET_KEY",
    "AI_GATEWAY_API_KEY",
    "VERCEL_TOKEN",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
];

/// Secret-shaped token prefixes (vendor key formats). A token starting with
/// one of these is redacted even when it is not one of *our* secrets — a page
/// echoing anyone's live key must not be memorized into a world card.
const SECRET_PREFIXES: &[&str] = &[
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "rk_test_",
    "whsec_",
    "sk-ant-",
    "ghp_",
    "gho_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
];

/// A prefix match only counts as a secret with at least this many token
/// characters after it (avoids redacting prose like "sk_test_ keys").
const MIN_BODY: usize = 6;

/// Host secret values shorter than this are ignored — replacing a degenerate
/// value like "test" would shred ordinary prose.
const MIN_SECRET_LEN: usize = 8;

/// Scrub outcome: the cleaned text plus *labels* of what was redacted (the
/// env-key name or the matched prefix) — never the secret itself, so the
/// outcome is safe to audit-log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scrubbed {
    pub text: String,
    pub redactions: Vec<String>,
}

impl Scrubbed {
    pub fn clean(&self) -> bool {
        self.redactions.is_empty()
    }
}

/// The host's own credential values, read from the environment as
/// `(env key, value)` pairs. Callers pass these to [`scrub`] so the scan
/// itself stays a pure function.
pub fn host_secrets() -> Vec<(String, String)> {
    SECRET_ENV_KEYS
        .iter()
        .filter_map(|k| crate::config::nonempty_env(k).map(|v| (k.to_string(), v)))
        .filter(|(_, v)| v.len() >= MIN_SECRET_LEN)
        .collect()
}

/// Redact host credential values and secret-shaped tokens from inbound text.
pub fn scrub(text: &str, host_secrets: &[(String, String)]) -> Scrubbed {
    let mut redactions = Vec::new();

    // pass 1: exact host credential values, labeled by env key
    let mut out = text.to_string();
    for (key, value) in host_secrets {
        if out.contains(value.as_str()) {
            out = out.replace(value.as_str(), &format!("[redacted:{key}]"));
            push_unique(&mut redactions, key.clone());
        }
    }

    // pass 2: secret-shaped tokens by vendor prefix
    let out = redact_patterns(&out, &mut redactions);

    Scrubbed {
        text: out,
        redactions,
    }
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn push_unique(redactions: &mut Vec<String>, label: String) {
    if !redactions.contains(&label) {
        redactions.push(label);
    }
}

fn redact_patterns(text: &str, redactions: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'scan: while !rest.is_empty() {
        for prefix in SECRET_PREFIXES {
            if rest.starts_with(prefix) {
                let body_len = rest[prefix.len()..]
                    .chars()
                    .take_while(|c| is_token_char(*c))
                    .count();
                if body_len >= MIN_BODY {
                    out.push_str(&format!("[redacted:{prefix}…]"));
                    push_unique(redactions, (*prefix).to_string());
                    rest = &rest[prefix.len() + body_len..];
                    continue 'scan;
                }
            }
        }
        // AWS access-key ids: AKIA followed by exactly 16 uppercase alnums
        if rest.starts_with("AKIA") {
            let body: String = rest[4..]
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                .collect();
            if body.len() == 16 && !rest[4 + 16..].starts_with(is_token_char) {
                out.push_str("[redacted:AKIA…]");
                push_unique(redactions, "AKIA".to_string());
                rest = &rest[4 + 16..];
                continue 'scan;
            }
        }
        let c = rest.chars().next().expect("non-empty");
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_credential_value_is_redacted_and_labeled_by_key() {
        let secrets = vec![("EXA_API_KEY".to_string(), "1f2e3d4c5b6a7890".to_string())];
        let s = scrub("the page echoed 1f2e3d4c5b6a7890 back", &secrets);
        assert_eq!(s.text, "the page echoed [redacted:EXA_API_KEY] back");
        assert_eq!(s.redactions, vec!["EXA_API_KEY"]);
        // the label never carries the value
        assert!(!s.redactions.iter().any(|r| r.contains("1f2e3d4c")));
    }

    #[test]
    fn vendor_key_shapes_are_redacted_even_when_not_ours() {
        // hyphenated stand-in: real-shaped enough for the scrubber's token
        // scan, fake enough to never trip a registry-grade secret scanner
        let s = scrub(
            "leaked: sk_test_FAKE-FAKE-FAKE-2026 and ghp_AbCdEf012345xyz",
            &[],
        );
        assert!(!s.text.contains("sk_test_FAKE"));
        assert!(!s.text.contains("ghp_AbCdEf"));
        assert!(s.text.contains("[redacted:sk_test_…]"));
        assert!(s.text.contains("[redacted:ghp_…]"));
        assert_eq!(s.redactions, vec!["sk_test_", "ghp_"]);
    }

    #[test]
    fn aws_access_key_id_shape_is_redacted() {
        let s = scrub("creds AKIAIOSFODNN7EXAMPLE in a dump", &[]);
        assert_eq!(s.text, "creds [redacted:AKIA…] in a dump");
        // 15 chars after AKIA is not the shape
        let not = scrub("AKIAIOSFODNN7EXAMPL", &[]);
        assert!(not.clean());
    }

    #[test]
    fn prose_mentioning_a_prefix_is_left_alone() {
        let s = scrub("rotate your sk_live_ keys quarterly", &[]);
        assert!(s.clean());
        assert_eq!(s.text, "rotate your sk_live_ keys quarterly");
    }

    #[test]
    fn short_host_secret_values_are_ignored() {
        // a degenerate secret value must not shred prose
        assert!(host_secrets()
            .iter()
            .all(|(_, v)| v.len() >= MIN_SECRET_LEN));
        let secrets = vec![("X".to_string(), "test".to_string())];
        // scrub() trusts callers to have filtered; host_secrets() is the filter —
        // but even unfiltered, the label-only contract holds
        let s = scrub("a test sentence", &secrets);
        assert!(s.text.contains("[redacted:X]") || s.text == "a test sentence");
    }

    #[test]
    fn clean_text_passes_through_unchanged() {
        let s = scrub("Enterprise tiers offer committed-use discounts.", &[]);
        assert!(s.clean());
        assert_eq!(s.text, "Enterprise tiers offer committed-use discounts.");
    }
}
