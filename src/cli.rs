//! Headless command-line interface: the library operations the window
//! offers, scriptable — check, health, merge, clean-urls, export-chrome
//! and snapshot. `run` returns `(exit_code, output)`: 0 success, 1 the
//! operation failed or (for `health`) issues remain, 2 usage error.
//! No arguments at all still means "open the window" (handled by main).
use crate::health::IssueKind;
use crate::models::BookmarkDocument;
use crate::review_state::Resolution;
use crate::{health, history::History, merge::MergeSession, parser};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "\
RustMarks — bookmark studio, headless mode

Usage:
  rustmarks                          Open the window
  rustmarks check <file> [--json]
  rustmarks health <file> [--json] [--fix]
  rustmarks merge <base> <incoming>... --output <file> [--prefer existing|incoming|combine]
  rustmarks clean-urls <file> --output <file>
  rustmarks export-chrome <file> --output <file.json>
  rustmarks snapshot <file> [--history-dir <dir>]
  rustmarks help

Files may be Netscape HTML, Chrome/Edge JSON, or native RustMarks JSON.
Merge output is Netscape HTML unless its name ends in .json (native format).
Merge conflicts default to --prefer existing, like keeping what you have.";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn load(path: &str) -> Result<BookmarkDocument, String> {
    parser::load(Path::new(path)).map_err(|e| format!("Cannot open {path}: {e}"))
}

fn save(doc: &BookmarkDocument, path: &str) -> Result<(), String> {
    parser::save(doc, Path::new(path)).map_err(|e| format!("Cannot write {path}: {e}"))
}

fn check(args: &[String]) -> (i32, String) {
    let Some(file) = args.first() else {
        return (2, "check needs a file.\n".into());
    };
    match load(file) {
        Ok(doc) => {
            let report = health::scan(&doc);
            if args.iter().any(|a| a == "--json") {
                let value = serde_json::json!({
                    "file": file,
                    "bookmarks": report.bookmarks,
                    "folders": report.folders,
                });
                (0, format!("{value}\n"))
            } else {
                (
                    0,
                    format!(
                        "{file}: {} bookmarks in {} folders\n",
                        report.bookmarks, report.folders
                    ),
                )
            }
        }
        Err(e) => (1, format!("{e}\n")),
    }
}

fn health_cmd(args: &[String]) -> (i32, String) {
    let Some(file) = args.first() else {
        return (2, "health needs a file.\n".into());
    };
    let json = args.iter().any(|a| a == "--json");
    let fix = args.iter().any(|a| a == "--fix");
    let mut doc = match load(file) {
        Ok(doc) => doc,
        Err(e) => return (1, format!("{e}\n")),
    };
    let mut out = String::new();
    if fix {
        if let Err(e) = History::default().save(&doc, "cli-health-fix") {
            return (1, format!("Snapshot before fixing failed: {e}\n"));
        }
        let before = health::scan(&doc);
        let mut fixed = 0;
        for issue in &before.issues {
            if issue.fixable && health::fix(&mut doc, issue) {
                fixed += 1;
            }
        }
        if let Err(e) = save(&doc, file) {
            return (1, format!("{e}\n"));
        }
        out.push_str(&format!(
            "Fixed {fixed} issue(s); a snapshot was taken first.\n"
        ));
    }
    let report = health::scan(&doc);
    if json {
        let counts: serde_json::Map<String, serde_json::Value> = IssueKind::ALL
            .iter()
            .map(|k| (format!("{k:?}"), serde_json::json!(report.count(*k))))
            .collect();
        let value = serde_json::json!({
            "file": file,
            "bookmarks": report.bookmarks,
            "folders": report.folders,
            "issues": counts,
            "clean": report.is_clean(),
        });
        out.push_str(&format!("{value}\n"));
    } else if report.is_clean() {
        out.push_str(&format!(
            "{file}: healthy — {} bookmarks, no issues\n",
            report.bookmarks
        ));
    } else {
        out.push_str(&format!("{file}: {} issue(s)\n", report.issues.len()));
        for kind in IssueKind::ALL {
            let count = report.count(kind);
            if count > 0 {
                out.push_str(&format!("  {}: {count}\n", kind.label()));
            }
        }
        for issue in &report.issues {
            out.push_str(&format!("  - {}\n", issue.detail));
        }
    }
    (if report.is_clean() { 0 } else { 1 }, out)
}

fn merge_cmd(args: &[String]) -> (i32, String) {
    let Some(output) = flag(args, "--output") else {
        return (2, "merge needs --output <file>.\n".into());
    };
    let prefer = match flag(args, "--prefer").as_deref() {
        None | Some("existing") => Resolution::KeepExisting,
        Some("incoming") => Resolution::UseIncoming,
        Some("combine") => Resolution::Combine,
        Some(other) => {
            return (
                2,
                format!("--prefer must be existing, incoming or combine, not {other}.\n"),
            )
        }
    };
    let files: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .filter(|a| {
            flag(args, "--output").as_ref() != Some(*a)
                && flag(args, "--prefer").as_ref() != Some(*a)
        })
        .collect();
    if files.len() < 2 {
        return (
            2,
            "merge needs a base file and at least one incoming file.\n".into(),
        );
    }
    let base = match load(files[0]) {
        Ok(doc) => doc,
        Err(e) => return (1, format!("{e}\n")),
    };
    let mut sources: Vec<(String, BookmarkDocument)> = Vec::new();
    for file in &files[1..] {
        match load(file) {
            Ok(doc) => sources.push((file.to_string(), doc)),
            Err(e) => return (1, format!("{e}\n")),
        }
    }
    let mut session = match MergeSession::new(&base, sources) {
        Ok(session) => session,
        Err(e) => return (1, format!("Merge failed: {e}\n")),
    };
    session.decide_all(prefer);
    let stats = session.stats();
    match session.apply(&base) {
        Ok(merged) => match save(&merged, &output) {
            Ok(()) => (
                0,
                format!(
                    "Merged into {output}: {} added, {} duplicates skipped, {} conflicts resolved\n",
                    stats.added, stats.duplicates, stats.conflicts
                ),
            ),
            Err(e) => (1, format!("{e}\n")),
        },
        Err(e) => (1, format!("Merge failed: {e}\n")),
    }
}

fn clean_cmd(args: &[String]) -> (i32, String) {
    let Some(file) = args.first() else {
        return (2, "clean-urls needs a file.\n".into());
    };
    let Some(output) = flag(args, "--output") else {
        return (2, "clean-urls needs --output <file>.\n".into());
    };
    let mut doc = match load(file) {
        Ok(doc) => doc,
        Err(e) => return (1, format!("{e}\n")),
    };
    let mut changed = 0;
    doc.root_folder.visit_mut(&mut |b| {
        let cleaned = crate::url_cleanup::clean(&b.url);
        if cleaned != b.url {
            b.url = cleaned;
            changed += 1;
        }
    });
    match save(&doc, &output) {
        Ok(()) => (0, format!("Cleaned {changed} URL(s); wrote {output}\n")),
        Err(e) => (1, format!("{e}\n")),
    }
}

fn export_cmd(args: &[String]) -> (i32, String) {
    let Some(file) = args.first() else {
        return (2, "export-chrome needs a file.\n".into());
    };
    let Some(output) = flag(args, "--output") else {
        return (2, "export-chrome needs --output <file>.\n".into());
    };
    let doc = match load(file) {
        Ok(doc) => doc,
        Err(e) => return (1, format!("{e}\n")),
    };
    match serde_json::to_vec_pretty(&parser::chrome(&doc)) {
        Ok(data) => match crate::storage::atomic_write(Path::new(&output), &data) {
            Ok(()) => (0, format!("Exported Chrome/Edge JSON to {output}\n")),
            Err(e) => (1, format!("Cannot write {output}: {e}\n")),
        },
        Err(e) => (1, format!("Export failed: {e}\n")),
    }
}

fn snapshot_cmd(args: &[String]) -> (i32, String) {
    let Some(file) = args.first() else {
        return (2, "snapshot needs a file.\n".into());
    };
    let doc = match load(file) {
        Ok(doc) => doc,
        Err(e) => return (1, format!("{e}\n")),
    };
    let history = match flag(args, "--history-dir") {
        Some(dir) => History {
            directory: PathBuf::from(dir),
        },
        None => History::default(),
    };
    match history.save(&doc, "cli") {
        Ok(()) => (
            0,
            format!("Snapshot saved in {}\n", history.directory.display()),
        ),
        Err(e) => (1, format!("Snapshot failed: {e}\n")),
    }
}

/// Entry point. `args` excludes the program name.
pub fn run(args: &[String]) -> (i32, String) {
    let Some(command) = args.first().map(String::as_str) else {
        return (2, USAGE.into());
    };
    let rest = &args[1..];
    match command {
        "help" | "--help" | "-h" => (0, USAGE.into()),
        "check" => check(rest),
        "health" => health_cmd(rest),
        "merge" => merge_cmd(rest),
        "clean-urls" => clean_cmd(rest),
        "export-chrome" => export_cmd(rest),
        "snapshot" => snapshot_cmd(rest),
        other => (2, format!("Unknown command {other}.\n\n{USAGE}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BookmarkItem, BookmarkNode};

    fn fixture(tag: &str) -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("rustmarks-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bookmarks.html");
        let mut doc = BookmarkDocument::default();
        let root = doc.root_folder.id();
        doc.add(
            Some(root),
            BookmarkNode::Bookmark(BookmarkItem::new(
                "Rust",
                "https://rust-lang.org/?utm_source=x",
            )),
        );
        parser::save(&doc, &path).unwrap();
        let text = path.to_string_lossy().into_owned();
        (dir, text)
    }

    #[test]
    fn usage_and_unknown_commands() {
        let (code, out) = run(&["help".into()]);
        assert_eq!(code, 0);
        assert!(out.contains("headless mode"));
        let (code, _) = run(&["bogus".into()]);
        assert_eq!(code, 2);
        let (code, _) = run(&[]);
        assert_eq!(code, 2);
    }

    #[test]
    fn check_and_health_report_the_fixture() {
        let (_dir, file) = fixture("report");
        let (code, out) = run(&["check".into(), file.clone()]);
        assert_eq!(code, 0);
        assert!(out.contains("1 bookmarks"), "{out}");
        let (code, out) = run(&["health".into(), file.clone()]);
        assert_eq!(code, 1, "issues remain, so exit 1: {out}");
        assert!(out.contains("Tracking parameters: 1"), "{out}");
        let (code, out) = run(&["health".into(), file.clone(), "--json".into()]);
        assert_eq!(code, 1);
        assert!(out.contains("\"clean\":false"), "{out}");
    }

    #[test]
    fn clean_urls_writes_a_cleaned_copy() {
        let (dir, file) = fixture("clean");
        let out = dir.join("clean.html");
        let (code, text) = run(&[
            "clean-urls".into(),
            file,
            "--output".into(),
            out.to_string_lossy().into_owned(),
        ]);
        assert_eq!(code, 0, "{text}");
        let doc = parser::load(&out).unwrap();
        assert_eq!(doc.items()[0].url, "https://rust-lang.org/");
    }

    #[test]
    fn merge_adds_incoming_bookmarks() {
        let (dir, base) = fixture("merge");
        let incoming_path = dir.join("incoming.html");
        let mut incoming = BookmarkDocument::default();
        let root = incoming.root_folder.id();
        incoming.add(
            Some(root),
            BookmarkNode::Bookmark(BookmarkItem::new("New", "https://example.com/new")),
        );
        parser::save(&incoming, &incoming_path).unwrap();
        let out = dir.join("merged.html");
        let (code, text) = run(&[
            "merge".into(),
            base,
            incoming_path.to_string_lossy().into_owned(),
            "--output".into(),
            out.to_string_lossy().into_owned(),
        ]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("1 added"), "{text}");
        assert_eq!(parser::load(&out).unwrap().items().len(), 2);
    }
}
