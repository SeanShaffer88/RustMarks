# Project Blueprint: RustMarks — The VS Code-Themed Bookmark Studio for Windows

## 1. Executive Summary & Project Scope

**RustMarks** is a native, offline-first Windows desktop utility written entirely in Rust using the `egui`/`eframe` immediate-mode GUI framework. It mimics the look, feel, layout, and keyboard-driven ergonomics of Visual Studio Code (Dark Modern theme).

### Core Capabilities:

* **Universal Parser & Standardizer:** Imports/exports Netscape HTML bookmarks (Firefox/Chrome/Safari legacy) and JSON bookmark trees (Chrome/Edge), standardizing them into an internal unified object model backed by SQLite/JSON storage.

* **Granular & Bulk Editing:** Features a split-view workspace combining an expandable Tree Explorer and an Excel-like Inline Spreadsheet Grid Editor for bulk tagging, URL correction, note-taking, and favicon management.

* **Smart Heuristic Categorization:** A robust local rule-based and taxonomy-driven engine that auto-classifies bookmarks based on domain parsing, path heuristics, keywords, and extensible category dictionaries.

* **Advanced File Operations:** Seamless merging of multiple bookmark files with deduplication algorithms, conflict resolution wizards, and incremental bookmark additions.

* **Before/After Staged Diff Engine:** A robust staging workflow allowing users to toggle between a side-by-side tree diff view and a unified inline diff view to inspect all modifications before committing to disk.

* **Automatic Versioning & Rollback:** Local automatic snapshots maintaining a journal of changes, permitting instant roll-forward/roll-back safety.

## 2. Visual Studio Code Theme & UI Design System

To achieve an authentic Visual Studio Code aesthetic in `egui`, the application overrides the default style and visual palette.

### Color Palette (`vscode_theme.rs`):

```rust
pub struct VSCodeTheme {
    pub bg_deep: egui::Color32,      // #181818 - Activity Bar / Side Bar Background
    pub bg_editor: egui::Color32,    // #1e1e1e - Main Editor Area Background
    pub bg_surface: egui::Color32,   // #252526 - Panels, Tabs, Input Fields
    pub bg_hover: egui::Color32,     // #2a2d2e - List/Tree Item Hover
    pub bg_selected: egui::Color32,  // #37373d - Active Selection Highlight
    pub border: egui::Color32,       // #333333 - Dividers and Panel Borders
    pub text_primary: egui::Color32, // #cccccc - Primary Labels & Text
    pub text_muted: egui::Color32,   // #858585 - Placeholders, File Paths, Icons
    pub accent: egui::Color32,       // #007acc - VS Code Blue (Focus, Primary Buttons)
    pub green: egui::Color32,        // #4ec9b0 - Added items in diff / Success tags
    pub red: egui::Color32,          // #f14c4c - Deleted items in diff / Warnings
    pub orange: egui::Color32,       // #ce9178 - String literals / URLs
}
```

### Layout Anatomy:

1. **Activity Bar (Leftmost narrow strip):** Icon buttons for switching primary views: Explorer, Smart Organizer, Merge Studio, Version History, Settings.

2. **Primary Sidebar:** Tree view of the active bookmarks file, folder hierarchy, and quick filter search box.

3. **Editor Area (Center):** Tabbed workspace supporting the Inline Spreadsheet Grid, Bookmark Details Editor, and Before/After Diff Viewer.

4. **Status Bar (Bottom strip):** Displays current file path, total bookmark count, dirty state indicator (`[Modified]`), active heuristic ruleset status, and encoding/format indicator (`UTF-8 / Netscape HTML`).

## 3. Technical Architecture & Rust Crate Stack

### Core Dependencies (`Cargo.toml`):

```toml
[package]
name = "rustmarks"
version = "0.1.0"
edition = "2021"

[dependencies]
eframe = "0.28"
egui = "0.28"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
scraper = "0.19"     # For parsing Netscape HTML bookmark files
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.8", features = ["v4", "serde"] }
rusqlite = { version = "0.31", features = ["bundled"] }
url = "2.5"
diff = "0.1.13"     # For structural diffing
image = "0.25"      # For favicon processing/caching
rfd = "0.14"        # Native Windows file dialogs
```

### Data Model (`models.rs`):

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BookmarkItem {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub favicon_base64: Option<String>,
    pub notes: String,
    pub tags: Vec<String>,
    pub date_added: i64,
    pub category_guess: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum BookmarkNode {
    Folder {
        id: Uuid,
        title: String,
        children: Vec<BookmarkNode>,
        date_added: i64,
    },
    Bookmark(BookmarkItem),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BookmarkDocument {
    pub version: u32,
    pub root_folder: BookmarkNode,
    pub source_path: Option<String>,
}
```

## 4. Feature Modules & Workflow Specification

### A. Parser & Standardizer Engine (`parser.rs`)

* **Netscape HTML Import:** Uses `scraper` to parse `<DT><A HREF="..." ADD_DATE="...">` tags and reconstruct recursive folder trees from `<DL><p>` blocks.

* **JSON Import:** Handles standard Chrome/Edge JSON exports (`roots.bookmark_bar`, `roots.other`).

* **Standardization:** Normalizes all parsed entries into the unified `BookmarkDocument` structure, extracting domains via the `url` crate for future heuristics.

### B. Inline Spreadsheet & Grid Editor (`editor.rs`)

* Replaces standard forms with a high-density, keyboard-navigable grid view inspired by VS Code settings/tables.

* **Bulk Operations:** Multi-select rows using `Ctrl`/`Shift` to apply batch tags, bulk delete, or batch domain cleanups.

* **Live Search & Filter:** Instant filtering across titles, URLs, tags, and notes using an expression filter bar (e.g., `tag:dev -broken`).

### C. Smart Heuristic Categorization Engine (`heuristics.rs`)

An expanded local rule-based database matching URL strings, domain suffixes, and path tokens against categorized dictionaries:

* *Developer & Tech:* `github.com`, `stackoverflow.com`, `crates.io`, `docs.rs`, keywords: `api`, `code`, `rust`, `compiler`.

* *Design & UI:* `figma.com`, `dribbble.com`, `behance.net`, keywords: `ui`, `ux`, `design`.

* *News & Reading:* `news.ycombinator.com`, `medium.com`, `substack.com`.

* *Productivity & Tools:* `notion.so`, `linear.app`, `figma.com`.

* *Execution:* Scans the bookmark list, matches tokens against weights, and populates `category_guess`, allowing users to accept single suggestions or execute a "One-Click Auto-Categorize into Folders" workflow.

### D. Merge & Deduplication Engine (`merge.rs`)

* Takes two or more `BookmarkDocument` instances.

* **Deduplication Strategy:** Exact URL matching and normalization (stripping tracking parameters like `utm_source`).

* **Conflict Resolution:** If titles or metadata differ for the same URL, presents a merge preview dialog allowing the user to select the primary record or combine tags/notes.

### E. Staged Diff & Preview Viewer (`diff_view.rs`)

* Maintains an `original_snapshot` and a `current_state`.

* **Side-by-Side View (Default):** Left tree shows original structure; Right tree shows modified structure with additions colored in green, deletions in red, and modifications highlighted.

* **Unified View (Toggleable):** A flattened list highlighting modified leaf nodes and structural folder shifts.

* **Commit / Revert:** Committing writes to the active file and creates an automatic backup snapshot; Reverting restores the original snapshot.

### F. Automated Versioning & Snapshots (`history.rs`)

* Automatically saves compressed JSON snapshots of bookmarks into a local application data folder (`%APPDATA%\RustMarks\history\`) prior to any major operation (merge, batch edit, file save).

* Provides a built-in History Explorer panel enabling users to time-travel, inspect previous states, and instantly roll back.

## 5. Step-by-Step Implementation Roadmap

1. **Phase 1: Project Scaffold & Theme Setup**
   * Initialize Rust cargo project (`rustmarks`), configure `eframe` and `egui`.
   * Implement `VSCodeTheme` palette and apply custom window styling.

2. **Phase 2: Data Models & Parsers**
   * Build Netscape HTML parser and Chrome JSON parser.
   * Implement serialization/deserialization tests.

3. **Phase 3: Core UI Workspaces**
   * Build Activity Bar navigation state.
   * Construct the Tree Explorer sidebar with folder expansion/collapse logic.

4. **Phase 4: Inline Grid Editor & Item Management**
   * Implement the spreadsheet table view for folder contents.
   * Add inline editing fields for title, URL, notes, and tag chips.

5. **Phase 5: Heuristics Engine & Merge Utilities**
   * Implement domain/keyword ruleset database.
   * Build merge dialog and URL normalization logic.

6. **Phase 6: Diff Viewer & Versioning**
   * Build structural tree diff calculation algorithm.
   * Implement side-by-side vs. unified toggle rendering and SQLite/file snapshot roll-back.