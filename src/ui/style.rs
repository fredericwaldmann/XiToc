//! XiToc design tokens. See docs/UI_DESIGN.md before extending the interface.
use egui::{Color32, FontFamily, FontId, Margin, Stroke, TextStyle};

pub const CONTROL: f32 = 26.0;
pub const ROW: f32 = 26.0;
pub const TAB: f32 = 36.0;
pub const GAP: f32 = 8.0;
pub const PAGE_GAP: f32 = 24.0;

pub use crate::settings::Theme;
pub const SMOOTHING_SLIDER_WIDTH: f32 = 200.0;
pub const SETTINGS_WIDTH: f32 = 520.0;

#[derive(Clone, Copy)]
pub struct Palette {
    pub shell: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub hover: Color32,
    pub workspace: Color32,
    pub board: Color32,
    pub dot: Color32,
}

impl Theme {
    pub fn palette(self) -> Palette {
        match self {
            Self::Light => Palette {
                shell: rgb(246, 247, 249),
                sidebar: rgb(240, 242, 245),
                surface: Color32::WHITE,
                text: rgb(32, 39, 49),
                muted: rgb(99, 112, 131),
                border: rgb(217, 222, 230),
                accent: rgb(37, 107, 104),
                selection: rgb(221, 237, 233),
                hover: rgb(229, 233, 239),
                workspace: rgb(223, 227, 233),
                board: rgb(246, 248, 250),
                dot: rgb(198, 207, 217),
            },
            Self::Dark => Palette {
                shell: rgb(27, 32, 40),
                sidebar: rgb(24, 29, 36),
                surface: rgb(36, 43, 53),
                text: rgb(230, 234, 240),
                muted: rgb(156, 168, 185),
                border: rgb(53, 62, 74),
                accent: rgb(115, 199, 188),
                selection: rgb(41, 68, 63),
                hover: rgb(47, 56, 69),
                workspace: rgb(16, 21, 28),
                board: rgb(27, 33, 42),
                dot: rgb(64, 76, 92),
            },
        }
    }

    pub fn apply(self, ctx: &egui::Context) {
        let id = egui::Id::unique("xitoc-theme");
        if ctx.data(|data| data.get_temp::<Theme>(id)) == Some(self) {
            return;
        }
        let p = self.palette();
        let mut visuals = match self {
            Self::Light => egui::Visuals::light(),
            Self::Dark => egui::Visuals::dark(),
        };
        visuals.override_text_color = Some(p.text);
        visuals.weak_text_color = Some(p.muted);
        visuals.panel_fill = p.shell;
        visuals.window_fill = p.surface;
        visuals.extreme_bg_color = p.surface;
        visuals.faint_bg_color = p.sidebar;
        visuals.hyperlink_color = p.accent;
        visuals.selection.bg_fill = p.selection;
        visuals.selection.stroke = Stroke::new(1.0, p.accent);
        visuals.window_corner_radius = egui::CornerRadius::same(6);
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.noninteractive,
        ] {
            widget.bg_fill = p.surface;
            widget.weak_bg_fill = p.shell;
            widget.bg_stroke = Stroke::new(1.0, p.border);
            widget.fg_stroke = Stroke::new(1.0, p.text);
            widget.corner_radius = egui::CornerRadius::same(4);
        }
        visuals.widgets.hovered.bg_fill = p.hover;
        visuals.widgets.hovered.weak_bg_fill = p.hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
        visuals.widgets.active.bg_fill = p.selection;
        visuals.widgets.active.weak_bg_fill = p.selection;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.accent);
        ctx.set_theme(match self {
            Self::Light => egui::Theme::Light,
            Self::Dark => egui::Theme::Dark,
        });
        ctx.set_visuals(visuals);
        ctx.global_style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(GAP, 6.0);
            style.spacing.button_padding = egui::vec2(8.0, 4.0);
            style.spacing.interact_size.y = CONTROL;
            style.spacing.indent = 16.0;
            style
                .text_styles
                .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
            style.text_styles.insert(
                TextStyle::Button,
                FontId::new(13.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Heading,
                FontId::new(18.0, FontFamily::Proportional),
            );
        });
        ctx.data_mut(|data| data.insert_temp(id, self));
    }
}

pub fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode {
        Theme::Dark.palette()
    } else {
        Theme::Light.palette()
    }
}

pub fn frame(color: Color32, x: i8, y: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(color)
        .inner_margin(Margin::symmetric(x, y))
}
const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
