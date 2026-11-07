use crate::models::*;
use crate::rules::RuleSet;
pub fn classify(b: &BookmarkItem) -> Option<String> {
    RuleSet::default().classify(b)
}
pub fn suggest_with(doc: &mut BookmarkDocument, rules: &RuleSet) {
    doc.root_folder
        .visit_mut(&mut |b| b.category_guess = rules.classify(b));
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
