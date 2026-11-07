//! User-editable, deterministic local categorization rules.
use crate::{models::BookmarkItem, parser::Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CategoryRule {
    pub category: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "domain_weight")]
    pub domain_weight: u32,
    #[serde(default = "keyword_weight")]
    pub keyword_weight: u32,
}
fn domain_weight() -> u32 {
    10
}
fn keyword_weight() -> u32 {
    1
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleSet {
    pub version: u32,
    pub rules: Vec<CategoryRule>,
}
impl Default for RuleSet {
    fn default() -> Self {
        let entries: &[(&str, &[&str], &[&str])] = &[
            (
                "Developer & Tech",
                &[
                    "github.com",
                    "stackoverflow.com",
                    "crates.io",
                    "docs.rs",
                    "developer.mozilla.org",
                ],
                &["api", "code", "rust", "compiler"],
            ),
            (
                "Design & UI",
                &["figma.com", "dribbble.com", "behance.net"],
                &["ui", "ux", "design"],
            ),
            (
                "News & Reading",
                &["news.ycombinator.com", "medium.com", "substack.com"],
                &["news", "article", "reading"],
            ),
            (
                "Productivity & Tools",
                &["notion.so", "linear.app"],
                &["productivity", "workflow", "tasks"],
            ),
        ];
        Self {
            version: 1,
            rules: entries
                .iter()
                .map(|(category, domains, keywords)| CategoryRule {
                    category: (*category).into(),
                    domains: domains.iter().map(|s| (*s).into()).collect(),
                    keywords: keywords.iter().map(|s| (*s).into()).collect(),
                    domain_weight: 10,
                    keyword_weight: 1,
                })
                .collect(),
        }
    }
}
impl RuleSet {
    pub fn parse(text: &str) -> Result<Self> {
        let rules: Self = serde_json::from_str(text)?;
        if rules.version != 1 {
            return Err("Unsupported ruleset version".into());
        }
        if rules.rules.iter().any(|r| {
            r.category.trim().is_empty()
                || r.domains
                    .iter()
                    .any(|d| d.trim().is_empty() || d.contains('/') || d.contains(':'))
                || r.keywords.iter().any(|k| k.trim().is_empty())
        }) {
            return Err("Rules need a category and nonempty host names / keywords (domains must not include a scheme or path)".into());
        }
        Ok(rules)
    }
    pub fn classify(&self, bookmark: &BookmarkItem) -> Option<String> {
        let parsed = url::Url::parse(&bookmark.url).ok();
        let host = parsed.as_ref().and_then(|u| u.host_str()).unwrap_or("");
        let text = format!(
            "{} {} {} {}",
            bookmark.title,
            parsed.as_ref().map(|u| u.path()).unwrap_or(""),
            bookmark.tags.join(" "),
            bookmark.notes
        )
        .to_lowercase();
        let tokens: Vec<_> = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();
        let mut winner = None;
        let mut best = 0u64;
        // Earlier rules win ties, making overlapping domains predictable.
        for rule in &self.rules {
            let domain_score = rule
                .domains
                .iter()
                .filter(|d| {
                    let d = d.trim().to_lowercase();
                    host == d || host.ends_with(&format!(".{d}"))
                })
                .count() as u64
                * u64::from(rule.domain_weight);
            let keyword_score = rule
                .keywords
                .iter()
                .filter(|k| tokens.contains(&k.to_lowercase().as_str()))
                .count() as u64
                * u64::from(rule.keyword_weight);
            let score = domain_score + keyword_score;
            if score > best {
                best = score;
                winner = Some(rule.category.clone());
            }
        }
        winner
    }
}
