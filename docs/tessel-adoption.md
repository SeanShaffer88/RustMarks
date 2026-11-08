# Tessel adoption — RustMarks

RustMarks conforms to the Tessel design system (SeanShaffer88/TesselGUI,
revision 1). Adoption followed Tessel's staged order: color roles, density,
layout, data lists, keyboard, states, then themes. Mockups built from real
screenshots were approved before implementation.

## What conforms

- **Color roles.** Every surface, text and state color comes from Tessel's
  generated token export (`dist/tessel.rs`), applied in `src/tessel.rs`.
  One crimson element per region: the region's primary action, the
  selection marker, or the 2px active-region rule under the active view's
  header.
- **Density.** Per-component by default (compact rows/toolbars, comfortable
  navigation); Compact, Comfortable and Touch are available in Settings and
  persist in `settings.json`.
- **Layout.** The explorer keeps one dominant region (the editor canvas)
  with a supporting navigation rail and explorer column; 1px `edge` strokes
  separate compartments.
- **Data lists.** The bookmark grid keeps its cell structure and striping;
  the selected row is painted with the `selected` fill and the selection
  indicator.
- **Keyboard.** Ctrl+K command palette (fuzzy search, disabled commands
  explain why, arrow keys + Enter, Esc closes), Ctrl+O / Ctrl+S /
  Ctrl+Shift+S, Ctrl+Z / Ctrl+Y, Ctrl+, for Settings, F1 for help,
  Ctrl+1…6 for the six views. A shortcut hint line sits in the status bar.
- **States.** Empty states carry the 24px grid motif plus a real message and
  action; destructive actions use danger text, never a filled brand button;
  status text always accompanies color.
- **Themes.** Dark and Light follow the OS setting live (via the
  `dark-light` crate — the one dependency added for this adoption), or can
  be fixed in Settings. Accent presets (Crimson, Blue, Teal, Graphite)
  recolor action, hover, pressed, selection indicator and focus together,
  exactly as Tessel's preset table specifies. Fonts: Segoe UI and Cascadia
  Mono / Consolas are loaded from the Windows Fonts folder at runtime on
  Windows; nothing is bundled.

## Recorded deviations (framework limits)

- **Focus outline.** Tessel specifies a 3px outline at a 3px offset; egui
  cannot draw an offset outline. Focus is approximated with a 1.5px stroke
  in the `focus` color on selections and the `selectionIndicator` stroke on
  active widgets.
- **F6 region cycling** is not implemented; egui region focus does not map
  to Tessel compartments. Ctrl+1…6 view keys and the palette cover the same
  navigation need.
- **Usage-ordered palette results.** The palette sorts by match quality and
  catalog order; per-user recent ordering is a named follow-up in Tessel's
  own palette spec, not part of revision 1 behavior here.
- **Wood-grain texture** is optional in Tessel and is not adopted: this is
  a data-dense tool, and Tessel's own guidance keeps texture subtle or off
  for such surfaces.
- **High-contrast / forced-colors** mode has no hook in egui 0.28; the
  Graphite preset is the low-chroma option Tessel provides for
  color-critical work.
