use crate::models::*;
const RULES: &[(&str, &[&str], &[&str])] = &[
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
pub fn classify(b: &BookmarkItem) -> Option<String> {
    let parsed = url::Url::parse(&b.url).ok();
    let host = parsed.as_ref().and_then(|u| u.host_str()).unwrap_or("");
    let text = format!(
        "{} {} {}",
        b.title,
        parsed.as_ref().map(|u| u.path()).unwrap_or(""),
        b.tags.join(" ")
    )
    .to_lowercase();
    let tokens: Vec<_> = text.split(|c: char| !c.is_alphanumeric()).collect();
    RULES
        .iter()
        .map(|(name, domains, keywords)| {
            let score = domains
                .iter()
                .filter(|d| host == **d || host.ends_with(&format!(".{d}")))
                .count()
                * 10
                + keywords.iter().filter(|k| tokens.contains(k)).count();
            (*name, score)
        })
        .filter(|(_, s)| *s > 0)
        .max_by_key(|(_, s)| *s)
        .map(|(n, _)| n.to_owned())
}
pub fn suggest(doc: &mut BookmarkDocument) {
    doc.root_folder
        .visit_mut(&mut |b| b.category_guess = classify(b));
}
pub fn organize(doc: &mut BookmarkDocument) {
    let mut buckets = std::collections::BTreeMap::<String, Vec<BookmarkNode>>::new();
    fn extract(
        n: &mut BookmarkNode,
        buckets: &mut std::collections::BTreeMap<String, Vec<BookmarkNode>>,
    ) {
        if let Some(children) = n.children_mut() {
            let mut retained = vec![];
            for mut child in std::mem::take(children) {
                if let BookmarkNode::Bookmark(b) = &child {
                    if let Some(category) = &b.category_guess {
                        buckets.entry(category.clone()).or_default().push(child);
                        continue;
                    }
                }
                extract(&mut child, buckets);
                retained.push(child);
            }
            *children = retained;
        }
    }
    extract(&mut doc.root_folder, &mut buckets);
    for (name, mut nodes) in buckets {
        let children = doc.root_folder.children_mut().unwrap();
        if let Some(BookmarkNode::Folder {
            children: existing, ..
        }) = children
            .iter_mut()
            .find(|n| matches!(n, BookmarkNode::Folder{title,..} if title == &name))
        {
            existing.append(&mut nodes);
        } else {
            let mut folder = BookmarkNode::folder(name);
            folder.children_mut().unwrap().append(&mut nodes);
            children.push(folder);
        }
    }
}
pub fn matches(b: &BookmarkItem, query: &str) -> bool {
    let text = format!("{} {} {} {}", b.title, b.url, b.notes, b.tags.join(" ")).to_lowercase();
    query.split_whitespace().all(|term| {
        let (neg, term) = term
            .strip_prefix('-')
            .map(|t| (true, t))
            .unwrap_or((false, term));
        let term = term.to_lowercase();
        let found = if let Some(tag) = term.strip_prefix("tag:") {
            b.tags.iter().any(|t| t.to_lowercase() == tag)
        } else {
            text.contains(&term)
        };
        found != neg
    })
}
