//! Tessel design system (SeanShaffer88/TesselGUI, revision 1) applied to egui.
//!
//! Token values come from Tessel's generated Rust export (`dist/tessel.rs`)
//! and the preset table in `demo/tokens.css`; they are not re-invented here.
//! The active palette and density metrics live in a small global because egui
//! paints immediate-mode widgets from many call sites; `apply` is the only
//! writer, called at startup and whenever the appearance settings change.
use egui::{Color32, Stroke};
use rustmarks::ui_settings::{AccentPreset, Density, ThemeMode, UiSettings};
use std::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub selected: Color32,
    pub foreground: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub accent_foreground: Color32,
    pub selection_indicator: Color32,
    pub focus: Color32,
    pub action: Color32,
    pub action_hover: Color32,
    pub action_pressed: Color32,
    pub action_foreground: Color32,
    pub progress: Color32,
    pub info: Color32,
    pub warning: Color32,
    pub success: Color32,
    pub danger: Color32,
    pub edge: Color32,
    pub grid: Color32,
    pub surround: Color32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// Minimum height for controls in toolbars and rows (Compact class).
    pub control_height: f32,
    /// Navigation item height (Comfortable class in Per-component mode).
    pub nav_height: f32,
    /// Data row height (Compact class in Per-component mode).
    pub row_height: f32,
    /// Padding inside cards and panels.
    pub panel_padding: f32,
    /// Height for the one primary action in a region.
    pub primary_height: f32,
}

impl Metrics {
    pub fn for_density(density: Density) -> Self {
        match density {
            Density::PerComponent => Self {
                control_height: 32.,
                nav_height: 40.,
                row_height: 28.,
                panel_padding: 12.,
                primary_height: 40.,
            },
            Density::Compact => Self {
                control_height: 32.,
                nav_height: 36.,
                row_height: 28.,
                panel_padding: 12.,
                primary_height: 40.,
            },
            Density::Comfortable => Self {
                control_height: 40.,
                nav_height: 40.,
                row_height: 36.,
                panel_padding: 16.,
                primary_height: 40.,
            },
            Density::Touch => Self {
                control_height: 48.,
                nav_height: 48.,
                row_height: 48.,
                panel_padding: 20.,
                primary_height: 48.,
            },
        }
    }
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The six accent-driven roles a preset recolors together.
struct AccentRoles {
    accent: Color32,
    action: Color32,
    action_hover: Color32,
    action_pressed: Color32,
    selection_indicator: Color32,
    focus: Color32,
}

fn accent_roles(preset: AccentPreset, dark: bool) -> AccentRoles {
    let (accent, action, hover, pressed, indicator, focus) = match (preset, dark) {
        (AccentPreset::Crimson, true) => {
            (0x76172A, 0xD02946, 0xD6304B, 0xCA2141, 0xDC2749, 0xFF4D73)
        }
        (AccentPreset::Crimson, false) => {
            (0x76172A, 0xB40234, 0xA3052F, 0x920629, 0x9E1534, 0xD61F45)
        }
        (AccentPreset::Blue, true) => (0x023C79, 0x036DCD, 0x1072D3, 0x0168C5, 0x5FA8FF, 0xACD1FF),
        (AccentPreset::Blue, false) => (0x023C79, 0x0059AA, 0x045099, 0x01478A, 0x1558A6, 0x327AD1),
        (AccentPreset::Teal, true) => (0x034541, 0x027B74, 0x048079, 0x02756F, 0x37B7AF, 0x8FE4D9),
        (AccentPreset::Teal, false) => (0x034541, 0x08645E, 0x075B55, 0x06514C, 0x00665F, 0x13877F),
        (AccentPreset::Graphite, true) => {
            (0x3C3C42, 0x6D6D74, 0x72727A, 0x686870, 0x94949C, 0xDCDCE3)
        }
        (AccentPreset::Graphite, false) => {
            (0x3C3C42, 0x585860, 0x505057, 0x47474E, 0x585860, 0x75757D)
        }
    };
    AccentRoles {
        accent: rgb(accent),
        action: rgb(action),
        action_hover: rgb(hover),
        action_pressed: rgb(pressed),
        selection_indicator: rgb(indicator),
        focus: rgb(focus),
    }
}

impl Palette {
    /// Full token palette for a resolved dark/light choice and accent preset.
    pub fn resolve(dark: bool, preset: AccentPreset) -> Self {
        let roles = accent_roles(preset, dark);
        let mut palette = if dark {
            Self {
                background: rgb(0x1F1F21),
                surface: rgb(0x202123),
                raised: rgb(0x222225),
                selected: rgb(0x28292B),
                foreground: rgb(0xF1F1F3),
                muted: rgb(0xB7B7C1),
                border: rgb(0x92929C),
                accent: rgb(0x76172A),
                accent_foreground: rgb(0xF1F1F3),
                selection_indicator: rgb(0xDC2749),
                focus: rgb(0xFF4D73),
                action: rgb(0xD02946),
                action_hover: rgb(0xD6304B),
                action_pressed: rgb(0xCA2141),
                action_foreground: rgb(0xFFFFFF),
                progress: rgb(0xB7B7C1),
                info: rgb(0x9FC9FF),
                warning: rgb(0xF2CD82),
                success: rgb(0x8FD4AD),
                danger: rgb(0xFFB1A8),
                edge: rgb(0x414144),
                grid: rgb(0x28282B),
                surround: rgb(0x262626),
            }
        } else {
            Self {
                background: rgb(0xEDEDF0),
                surface: rgb(0xFFFFFF),
                raised: rgb(0xE4E4E9),
                selected: rgb(0xDDDDE4),
                foreground: rgb(0x202023),
                muted: rgb(0x50505B),
                border: rgb(0x676772),
                accent: rgb(0x76172A),
                accent_foreground: rgb(0xF1F1F3),
                selection_indicator: rgb(0x9E1534),
                focus: rgb(0xD61F45),
                action: rgb(0xB40234),
                action_hover: rgb(0xA3052F),
                action_pressed: rgb(0x920629),
                action_foreground: rgb(0xFFFFFF),
                progress: rgb(0x50505B),
                info: rgb(0x174E85),
                warning: rgb(0x735100),
                success: rgb(0x236340),
                danger: rgb(0x9D2B1F),
                edge: rgb(0xB8B8BD),
                grid: rgb(0xF2F2F5),
                surround: rgb(0xD6D6D6),
            }
        };
        palette.accent = roles.accent;
        palette.action = roles.action;
        palette.action_hover = roles.action_hover;
        palette.action_pressed = roles.action_pressed;
        palette.selection_indicator = roles.selection_indicator;
        palette.focus = roles.focus;
        palette
    }
}

static CURRENT: RwLock<(Palette, Metrics)> = RwLock::new((
    Palette {
        background: rgb(0x1F1F21),
        surface: rgb(0x202123),
        raised: rgb(0x222225),
        selected: rgb(0x28292B),
        foreground: rgb(0xF1F1F3),
        muted: rgb(0xB7B7C1),
        border: rgb(0x92929C),
        accent: rgb(0x76172A),
        accent_foreground: rgb(0xF1F1F3),
        selection_indicator: rgb(0xDC2749),
        focus: rgb(0xFF4D73),
        action: rgb(0xD02946),
        action_hover: rgb(0xD6304B),
        action_pressed: rgb(0xCA2141),
        action_foreground: rgb(0xFFFFFF),
        progress: rgb(0xB7B7C1),
        info: rgb(0x9FC9FF),
        warning: rgb(0xF2CD82),
        success: rgb(0x8FD4AD),
        danger: rgb(0xFFB1A8),
        edge: rgb(0x414144),
        grid: rgb(0x28282B),
        surround: rgb(0x262626),
    },
    Metrics {
        control_height: 32.,
        nav_height: 40.,
        row_height: 28.,
        panel_padding: 12.,
        primary_height: 40.,
    },
));

/// The palette every widget paints with. Set by `apply`.
pub fn colors() -> Palette {
    CURRENT
        .read()
        .map(|guard| guard.0)
        .unwrap_or_else(|_| Palette::resolve(true, AccentPreset::Crimson))
}

/// The density metrics every widget lays out with. Set by `apply`.
pub fn metrics() -> Metrics {
    CURRENT
        .read()
        .map(|guard| guard.1)
        .unwrap_or_else(|_| Metrics::for_density(Density::PerComponent))
}

/// Resolve the effective dark/light choice for a theme mode. `System`
/// follows the OS; environments that report no preference get dark, the
/// design's reference theme.
pub fn resolve_dark(mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => !matches!(dark_light::detect(), dark_light::Mode::Light),
    }
}

/// Apply the settings to the egui context: token colors on every widget
/// state, Tessel spacing, and the Tessel type roles (Segoe UI is installed
/// separately by `install_fonts`).
pub fn apply(ctx: &egui::Context, settings: &UiSettings) {
    let dark = resolve_dark(settings.theme);
    let palette = Palette::resolve(dark, settings.accent);
    let metrics = Metrics::for_density(settings.density);
    if let Ok(mut current) = CURRENT.write() {
        *current = (palette, metrics);
    }
    let p = palette;
    let mut style = (*ctx.style()).clone();
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.panel_fill = p.background;
    style.visuals.window_fill = p.surface;
    style.visuals.extreme_bg_color = p.surface;
    style.visuals.faint_bg_color = p.raised;
    style.visuals.override_text_color = Some(p.foreground);
    style.visuals.selection.bg_fill = p.selected;
    style.visuals.selection.stroke = Stroke::new(1.5_f32, p.focus);
    style.visuals.window_stroke = Stroke::new(1.0_f32, p.edge);
    style.visuals.hyperlink_color = p.info;
    style.visuals.warn_fg_color = p.warning;
    style.visuals.error_fg_color = p.danger;
    let widgets = &mut style.visuals.widgets;
    widgets.noninteractive.bg_fill = p.background;
    widgets.noninteractive.weak_bg_fill = p.background;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.edge);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.muted);
    widgets.inactive.bg_fill = p.raised;
    widgets.inactive.weak_bg_fill = p.raised;
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.foreground);
    widgets.hovered.bg_fill = p.selected;
    widgets.hovered.weak_bg_fill = p.selected;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border);
    widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.foreground);
    widgets.active.bg_fill = p.selected;
    widgets.active.weak_bg_fill = p.selected;
    widgets.active.bg_stroke = Stroke::new(1.0_f32, p.selection_indicator);
    widgets.active.fg_stroke = Stroke::new(1.0_f32, p.foreground);
    widgets.open.bg_fill = p.selected;
    widgets.open.weak_bg_fill = p.selected;
    widgets.open.bg_stroke = Stroke::new(1.0_f32, p.border);
    widgets.open.fg_stroke = Stroke::new(1.0_f32, p.foreground);
    style.spacing.item_spacing = egui::vec2(8., 8.);
    style.spacing.button_padding = egui::vec2(12., 8.);
    style.spacing.interact_size.y = metrics.control_height;
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(24., FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(16., FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13., FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Small, FontId::new(11., FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13., FontFamily::Monospace),
    );
    ctx.set_style(style);
}

/// The 2px `selectionIndicator` rule under the active region's header.
/// One per window, drawn by the active view only.
pub fn active_rule(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 2.), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0_f32, colors().selection_indicator);
    ui.add_space(6.);
}

/// Paint the 24px Tessel grid motif inside `rect` (empty states and
/// canvases only, never behind text-dense content).
pub fn paint_grid(ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter().with_clip_rect(rect);
    let stroke = Stroke::new(1.0_f32, colors().grid);
    let mut x = rect.left() + (24. - rect.left() % 24.);
    while x < rect.right() {
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            stroke,
        );
        x += 24.;
    }
    let mut y = rect.top() + (24. - rect.top() % 24.);
    while y < rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
        y += 24.;
    }
}

/// Load the platform UI fonts Tessel names. On Windows that is Segoe UI
/// (proportional) and Cascadia Mono / Consolas (monospace), read from the
/// Windows Fonts folder at runtime — nothing is bundled or downloaded.
/// Other platforms keep egui's defaults (their system-ui equivalent).
pub fn install_fonts(ctx: &egui::Context) {
    #[cfg(target_os = "windows")]
    {
        let fonts_dir = std::env::var_os("WINDIR")
            .map(|w| std::path::PathBuf::from(w).join("Fonts"))
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows\Fonts"));
        let mut definitions = egui::FontDefinitions::default();
        let mut changed = false;
        if let Ok(bytes) = std::fs::read(fonts_dir.join("segoeui.ttf")) {
            definitions
                .font_data
                .insert("segoe-ui".into(), egui::FontData::from_owned(bytes));
            if let Some(family) = definitions
                .families
                .get_mut(&egui::FontFamily::Proportional)
            {
                family.insert(0, "segoe-ui".into());
                changed = true;
            }
        }
        for candidate in ["CascadiaMono.ttf", "CascadiaCode.ttf", "consola.ttf"] {
            if let Ok(bytes) = std::fs::read(fonts_dir.join(candidate)) {
                definitions
                    .font_data
                    .insert("tessel-mono".into(), egui::FontData::from_owned(bytes));
                if let Some(family) = definitions.families.get_mut(&egui::FontFamily::Monospace) {
                    family.insert(0, "tessel-mono".into());
                    changed = true;
                }
                break;
            }
        }
        if changed {
            ctx.set_fonts(definitions);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = ctx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_crimson_matches_the_tessel_tokens() {
        let p = Palette::resolve(true, AccentPreset::Crimson);
        assert_eq!(p.background, rgb(0x1F1F21));
        assert_eq!(p.surface, rgb(0x202123));
        assert_eq!(p.raised, rgb(0x222225));
        assert_eq!(p.selected, rgb(0x28292B));
        assert_eq!(p.edge, rgb(0x414144));
        assert_eq!(p.action, rgb(0xD02946));
        assert_eq!(p.action_hover, rgb(0xD6304B));
        assert_eq!(p.action_pressed, rgb(0xCA2141));
        assert_eq!(p.selection_indicator, rgb(0xDC2749));
        assert_eq!(p.focus, rgb(0xFF4D73));
        assert_eq!(p.accent, rgb(0x76172A));
    }

    #[test]
    fn light_crimson_uses_the_light_action_scale() {
        let p = Palette::resolve(false, AccentPreset::Crimson);
        assert_eq!(p.background, rgb(0xEDEDF0));
        assert_eq!(p.surface, rgb(0xFFFFFF));
        assert_eq!(p.action, rgb(0xB40234));
        assert_eq!(p.selection_indicator, rgb(0x9E1534));
    }

    #[test]
    fn presets_recolor_the_whole_accent_family_together() {
        let blue = Palette::resolve(true, AccentPreset::Blue);
        assert_eq!(blue.action, rgb(0x036DCD));
        assert_eq!(blue.selection_indicator, rgb(0x5FA8FF));
        assert_eq!(blue.focus, rgb(0xACD1FF));
        // Surfaces do not change with the preset.
        assert_eq!(
            blue.background,
            Palette::resolve(true, AccentPreset::Crimson).background
        );
        let graphite = Palette::resolve(true, AccentPreset::Graphite);
        assert_eq!(graphite.action, rgb(0x6D6D74));
        assert_eq!(graphite.selection_indicator, rgb(0x94949C));
    }

    #[test]
    fn density_metrics_follow_the_tessel_classes() {
        let per = Metrics::for_density(Density::PerComponent);
        assert_eq!(per.row_height, 28.);
        assert_eq!(per.nav_height, 40.);
        assert_eq!(per.control_height, 32.);
        let touch = Metrics::for_density(Density::Touch);
        assert_eq!(touch.row_height, 48.);
        assert_eq!(touch.control_height, 48.);
    }

    #[test]
    fn apply_sets_the_global_palette_and_panel_fill() {
        let context = egui::Context::default();
        apply(
            &context,
            &UiSettings {
                theme: ThemeMode::Dark,
                accent: AccentPreset::Crimson,
                density: Density::PerComponent,
                ..Default::default()
            },
        );
        assert_eq!(colors().action, rgb(0xD02946));
        assert_eq!(context.style().visuals.panel_fill, rgb(0x1F1F21));
        apply(
            &context,
            &UiSettings {
                theme: ThemeMode::Light,
                accent: AccentPreset::Crimson,
                density: Density::PerComponent,
                ..Default::default()
            },
        );
        assert_eq!(colors().background, rgb(0xEDEDF0));
        // Restore the dark default for other tests sharing this process.
        apply(&context, &UiSettings::default());
    }
}
