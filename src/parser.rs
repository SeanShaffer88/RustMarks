use crate::models::*;
use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn load(path: &Path) -> Result<BookmarkDocument> {
    if std::fs::metadata(path)?.len() > 64 * 1024 * 1024 {
        return Err("Bookmark file exceeds the 64 MiB import limit".into());
    }
    let text = std::fs::read_to_string(path)?;
    let mut doc = parse(&text)?;
    doc.source_path = Some(path.display().to_string());
    Ok(doc)
}
pub fn parse(text: &str) -> Result<BookmarkDocument> {
    let text = text.trim_start_matches('\u{feff}');
    if text.trim_start().starts_with('{') {
        let value: Value = serde_json::from_str(text)?;
        if value.get("root_folder").is_some() {
            let doc: BookmarkDocument = serde_json::from_value(value)?;
            doc.validate()?;
            return Ok(doc);
        }
        let roots = value
            .get("roots")
            .and_then(Value::as_object)
            .ok_or("JSON must contain Chrome roots or a RustMarks document")?;
        let mut doc = BookmarkDocument::default();
        for node in roots
            .values()
            .filter(|n| n.is_object() && n.get("type").is_some())
        {
            doc.add(None, chrome_node(node, 0)?);
        }
        return Ok(doc);
    }
    parse_html(text)
}
fn chrome_node(v: &Value, depth: usize) -> Result<BookmarkNode> {
    if depth > 128 {
        return Err("Folder nesting exceeds 128 levels".into());
    }
    if v["type"] != "url" && v["type"] != "folder" {
        return Err("Unsupported Chrome bookmark node type".into());
    }
    let title = v["name"].as_str().unwrap_or("Untitled");
    let date = v["date_added"]
        .as_str()
        .and_then(|s| s.parse::<i128>().ok())
        .and_then(|n| i64::try_from(n / 1_000_000 - 11_644_473_600).ok())
        .unwrap_or(0);
    if v["type"] == "url" {
        let mut b = BookmarkItem::new(title, v["url"].as_str().ok_or("Bookmark missing URL")?);
        b.date_added = date;
        Ok(BookmarkNode::Bookmark(b))
    } else {
        let mut node = BookmarkNode::folder(title);
        if let BookmarkNode::Folder {
            children,
            date_added,
            ..
        } = &mut node
        {
            *date_added = date;
            if let Some(entries) = v["children"].as_array() {
                for child in entries {
                    children.push(chrome_node(child, depth + 1)?);
                }
            }
        }
        Ok(node)
    }
}
// Scan structural tags in source order: HTML5 tree repair otherwise flattens Netscape's optional DT tags.
fn parse_html(text: &str) -> Result<BookmarkDocument> {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<dl") && !lower.contains("<a ") {
        return Err("Not a Netscape bookmark HTML file".into());
    }
    let mut stack = vec![BookmarkNode::folder("Bookmarks")];
    let mut pending: Option<BookmarkNode> = None;
    let mut outer = false;
    let mut pos = 0;
    while let Some(start) = lower[pos..].find('<').map(|n| n + pos) {
        let Some(end) = lower[start..].find('>').map(|n| n + start) else {
            break;
        };
        let tag = lower[start + 1..end]
            .split_whitespace()
            .next()
            .unwrap_or("");
        pos = end + 1;
        match tag {
            "h3" | "a" => {
                let close = format!("</{tag}>");
                if let Some(stop) = lower[pos..].find(&close).map(|n| n + pos) {
                    let frag = Html::parse_fragment(&text[start..stop + close.len()]);
                    let selector = Selector::parse(tag).unwrap();
                    if let Some(el) = frag.select(&selector).next() {
                        let title = el.text().collect::<String>();
                        let date = el
                            .value()
                            .attr("add_date")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        if tag == "h3" {
                            let mut n = BookmarkNode::folder(title);
                            if let BookmarkNode::Folder { date_added, .. } = &mut n {
                                *date_added = date;
                            }
                            pending = Some(n);
                        } else if let Some(href) = el.value().attr("href") {
                            let mut b = BookmarkItem::new(title, href);
                            b.date_added = date;
                            b.favicon_base64 = el.value().attr("icon").map(str::to_owned);
                            b.notes = el.value().attr("data-notes").unwrap_or("").to_owned();
                            b.tags = el
                                .value()
                                .attr("tags")
                                .unwrap_or("")
                                .split(',')
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned)
                                .collect();
                            stack
                                .last_mut()
                                .unwrap()
                                .children_mut()
                                .unwrap()
                                .push(BookmarkNode::Bookmark(b));
                        }
                    }
                    pos = stop + close.len();
                }
            }
            "dd" => {
                let stop = ["<dt", "</dl", "</dd"]
                    .iter()
                    .filter_map(|end| lower[pos..].find(end).map(|n| n + pos))
                    .min()
                    .unwrap_or(text.len());
                let fragment = Html::parse_fragment(&text[pos..stop]);
                let notes = fragment
                    .root_element()
                    .text()
                    .collect::<String>()
                    .trim()
                    .to_owned();
                if let Some(BookmarkNode::Bookmark(b)) = stack
                    .last_mut()
                    .and_then(|n| n.children_mut())
                    .and_then(|c| c.last_mut())
                {
                    if b.notes.is_empty() {
                        b.notes = notes;
                    }
                }
                pos = stop;
            }
            "dl" => {
                if let Some(folder) = pending.take() {
                    if stack.len() >= 128 {
                        return Err("Folder nesting exceeds 128 levels".into());
                    }
                    stack.push(folder);
                } else if !outer {
                    outer = true;
                }
            }
            "/dl" => {
                if stack.len() > 1 {
                    let n = stack.pop().unwrap();
                    stack.last_mut().unwrap().children_mut().unwrap().push(n);
                }
            }
            _ => {}
        }
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap();
        stack.last_mut().unwrap().children_mut().unwrap().push(n);
    }
    Ok(BookmarkDocument {
        root_folder: stack.pop().unwrap(),
        ..Default::default()
    })
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn html(doc: &BookmarkDocument) -> String {
    fn emit(n: &BookmarkNode, out: &mut String) {
        match n {
            BookmarkNode::Folder {
                title,
                children,
                date_added,
                ..
            } => {
                out.push_str(&format!(
                    "<DT><H3 ADD_DATE=\"{date_added}\">{}</H3>\n<DL><p>\n",
                    escape(title)
                ));
                for c in children {
                    emit(c, out);
                }
                out.push_str("</DL><p>\n");
            }
            BookmarkNode::Bookmark(b) => {
                out.push_str(&format!(
                    "<DT><A HREF=\"{}\" ADD_DATE=\"{}\" TAGS=\"{}\" DATA-NOTES=\"{}\"{}>{}</A>\n",
                    escape(&b.url),
                    b.date_added,
                    escape(&b.tags.join(",")),
                    escape(&b.notes),
                    b.favicon_base64
                        .as_ref()
                        .map(|s| format!(" ICON=\"{}\"", escape(s)))
                        .unwrap_or_default(),
                    escape(&b.title)
                ));
                if !b.notes.is_empty() {
                    out.push_str(&format!("<DD>{}\n", escape(&b.notes)));
                }
            }
        }
    }
    let mut out = "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<DL><p>\n".to_owned();
    if let BookmarkNode::Folder { children, .. } = &doc.root_folder {
        for c in children {
            emit(c, &mut out);
        }
    }
    out.push_str("</DL><p>\n");
    out
}
fn chrome_date(unix: i64) -> String {
    i128::from(unix)
        .saturating_add(11_644_473_600)
        .saturating_mul(1_000_000)
        .max(0)
        .to_string()
}
pub fn chrome(doc: &BookmarkDocument) -> Value {
    fn node(n: &BookmarkNode) -> Value {
        match n {
            BookmarkNode::Bookmark(b) => {
                json!({"type":"url", "name":b.title,"url":b.url,"date_added":chrome_date(b.date_added)})
            }
            BookmarkNode::Folder {
                title,
                children,
                date_added,
                ..
            } => {
                json!({"type":"folder","name":title,"children":children.iter().map(node).collect::<Vec<_>>(),"date_added":chrome_date(*date_added)})
            }
        }
    }
    json!({"version":1,"roots":{"bookmark_bar":node(&doc.root_folder)}})
}
pub fn save(doc: &BookmarkDocument, path: &Path) -> Result<()> {
    doc.validate()?;
    let data = if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("html") || s.eq_ignore_ascii_case("htm"))
    {
        html(doc)
    } else {
        serde_json::to_string_pretty(doc)?
    };
    crate::storage::atomic_write(path, data.as_bytes())
}
