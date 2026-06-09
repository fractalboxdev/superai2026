//! Human-readable Markdown cards — the source of truth for synthesized memory
//! (spec 02 §1). Each card carries frontmatter stamping its `acl_tag` so access
//! is all-or-nothing against the card's tag (prose can't be column-redacted).

use crate::connectors::AclTag;

/// Frontmatter metadata for a card.
pub struct CardMeta<'a> {
    pub topic: &'a str,
    pub kind: &'a str,
    pub period: Option<&'a str>,
    pub confidence: f32,
    pub acl_tag: &'a AclTag,
}

/// Render a card as Markdown with a YAML frontmatter block.
pub fn render_card(meta: &CardMeta, title: &str, body: &str) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("topic: {}\n", meta.topic));
    out.push_str(&format!("kind: {}\n", meta.kind));
    if let Some(p) = meta.period {
        out.push_str(&format!("period: {p}\n"));
    }
    out.push_str(&format!("confidence: {:.2}\n", meta.confidence));
    out.push_str(&format!("acl_view: {}\n", meta.acl_tag.view.id()));
    out.push_str(&format!(
        "acl_fields: [{}]\n",
        meta.acl_tag.fields.join(", ")
    ));
    out.push_str("---\n\n");
    out.push_str(&format!("# {title}\n\n"));
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A simple URL-safe slug for card filenames.
pub fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
