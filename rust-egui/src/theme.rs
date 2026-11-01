//! Colours and fonts for the three themes (Dark, Light and Box Box), with the
//! user's own colours from `general.colors`: `src/lib/theme.ts` and the
//! tokens in `src/styles/base.css`.

use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};

use crate::config::{General, ThemeId};

/// Font weights the widgets use (CSS 500, 600, 700, 800/900).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W {
    Medium,
    Semi,
    Bold,
    Heavy,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub id: ThemeId,
    pub ink: Color32,
    pub ink2: Color32,
    pub ink3: Color32,
    pub panel_solid: Color32,
    pub panel_raised: Color32,
    pub edge: Color32,
    pub wheel: Color32,
    pub fastest: Color32,
    pub gain: Color32,
    pub loss: Color32,
    pub attention: Color32,
    pub lapped: Color32,
    pub lapping: Color32,
    pub throttle: Color32,
    pub brake: Color32,
    pub clutch: Color32,
    // Mixed from the above, as in base.css.
    pub well: Color32,
    pub panel: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub grid: Color32,
    pub player: Color32,
    pub ghost_throttle: Color32,
    pub ghost_brake: Color32,
    pub ghost_attention: Color32,
    pub ghost_wheel: Color32,
    /// Box Box's broadcast red.
    pub f1_red: Color32,
    pub radius: f32,
    pub row_h: f32,
}

/// Every colour the user can change, with the colour it follows until changed.
pub const COLOR_KEYS: &[(&str, Option<&str>)] = &[
    ("ink", None),
    ("ink-2", None),
    ("ink-3", None),
    ("panel-solid", None),
    ("panel-raised", None),
    ("edge", None),
    ("wheel", None),
    ("fastest", None),
    ("gain", None),
    ("loss", None),
    ("attention", None),
    ("lapped", None),
    ("lapping", None),
    ("throttle", Some("gain")),
    ("brake", Some("loss")),
    ("clutch", None),
];

/// `THEME_COLORS` in theme.ts.
fn default_color(theme: ThemeId, key: &str) -> &'static str {
    let table: &[(&str, &str)] = match theme {
        ThemeId::Dark => &[
            ("ink", "#e9eef3"),
            ("ink-2", "#a7b4c2"),
            ("ink-3", "#6d7c8d"),
            ("panel-solid", "#0d131b"),
            ("panel-raised", "#151e29"),
            ("edge", "#92aac4"),
            ("wheel", "#c9d1da"),
            ("fastest", "#b98cff"),
            ("gain", "#4be08f"),
            ("loss", "#ff5468"),
            ("attention", "#ffb547"),
            ("lapped", "#6fb7ff"),
            ("lapping", "#ff9a6b"),
            ("throttle", "#4be08f"),
            ("brake", "#ff5468"),
            ("clutch", "#6fb7ff"),
        ],
        ThemeId::Light => &[
            ("ink", "#141c25"),
            ("ink-2", "#46525f"),
            ("ink-3", "#76828f"),
            ("panel-solid", "#f4f6f9"),
            ("panel-raised", "#ffffff"),
            ("edge", "#40556d"),
            ("wheel", "#3a4654"),
            ("fastest", "#8248e0"),
            ("gain", "#11a058"),
            ("loss", "#df2f47"),
            ("attention", "#d48300"),
            ("lapped", "#1c78d0"),
            ("lapping", "#dc6128"),
            ("throttle", "#11a058"),
            ("brake", "#df2f47"),
            ("clutch", "#1c78d0"),
        ],
        ThemeId::F1 => &[
            ("ink", "#ffffff"),
            ("ink-2", "#d2d2d8"),
            ("ink-3", "#8b8b96"),
            ("panel-solid", "#15151e"),
            ("panel-raised", "#2a2a33"),
            ("edge", "#9a9aa4"),
            ("wheel", "#d2d2d8"),
            ("fastest", "#b14ff0"),
            ("gain", "#2fd35f"),
            ("loss", "#ff1e1e"),
            ("attention", "#ffd21f"),
            ("lapped", "#3fa3ff"),
            ("lapping", "#ff8a3d"),
            ("throttle", "#2fd35f"),
            ("brake", "#ff1e1e"),
            ("clutch", "#3fa3ff"),
        ],
    };
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or("#808080")
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa` (the `#` optional).
pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    let digit = |i: usize, n: usize| u8::from_str_radix(s.get(i..i + n)?, 16).ok();
    match s.len() {
        3 => Some(Color32::from_rgb(digit(0, 1)? * 17, digit(1, 1)? * 17, digit(2, 1)? * 17)),
        6 => Some(Color32::from_rgb(digit(0, 2)?, digit(2, 2)?, digit(4, 2)?)),
        8 => Some(Color32::from_rgba_unmultiplied(digit(0, 2)?, digit(2, 2)?, digit(4, 2)?, digit(6, 2)?)),
        _ => None,
    }
}

/// A class or licence colour from the sim; `fallback` when it isn't one.
pub fn hex_or(s: &str, fallback: Color32) -> Color32 {
    parse_hex(s).unwrap_or(fallback)
}

/// `color-mix(in srgb, c p, transparent)`: the colour at `p` opacity.
pub fn alpha(c: Color32, p: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (a as f32 * p.clamp(0.0, 1.0)).round() as u8)
}

/// `color-mix(in srgb, a p, b)`.
pub fn mix(a: Color32, b: Color32, p: f32) -> Color32 {
    let [ar, ag, ab, aa] = a.to_srgba_unmultiplied();
    let [br, bg, bb, ba] = b.to_srgba_unmultiplied();
    let l = |x: u8, y: u8| (x as f32 * p + y as f32 * (1.0 - p)).round() as u8;
    Color32::from_rgba_unmultiplied(l(ar, br), l(ag, bg), l(ab, bb), l(aa, ba))
}

impl Theme {
    pub fn from_general(g: &General) -> Self {
        Self::build(g.theme, g.colors.get(&g.theme))
    }

    pub fn build(id: ThemeId, own: Option<&std::collections::HashMap<String, String>>) -> Self {
        fn effective(id: ThemeId, own: Option<&std::collections::HashMap<String, String>>, key: &str) -> Color32 {
            if let Some(c) = own.and_then(|o| o.get(key)).and_then(|s| parse_hex(s)) {
                return c;
            }
            if let Some((_, Some(follows))) = COLOR_KEYS.iter().find(|(k, _)| *k == key) {
                return effective(id, own, follows);
            }
            parse_hex(default_color(id, key)).unwrap_or(Color32::GRAY)
        }
        let c = |k: &str| effective(id, own, k);
        let (ink, ink3) = (c("ink"), c("ink-3"));
        let (throttle, brake, attention, wheel) = (c("throttle"), c("brake"), c("attention"), c("wheel"));
        let panel_solid = c("panel-solid");
        let edge = c("edge");
        let (well, panel_alpha, radius, row_h) = match id {
            ThemeId::Dark => (Color32::BLACK, 0.84, 6.0, 26.0),
            ThemeId::Light => (Color32::from_rgb(0x5d, 0x6f, 0x85), 0.90, 6.0, 26.0),
            ThemeId::F1 => (Color32::BLACK, 0.88, 0.0, 28.0),
        };
        Self {
            id,
            ink,
            ink2: c("ink-2"),
            ink3,
            panel_solid,
            panel_raised: c("panel-raised"),
            edge,
            wheel,
            fastest: c("fastest"),
            gain: c("gain"),
            loss: c("loss"),
            attention,
            lapped: c("lapped"),
            lapping: c("lapping"),
            throttle,
            brake,
            clutch: c("clutch"),
            well,
            panel: alpha(panel_solid, panel_alpha),
            line: alpha(edge, 0.16),
            line_strong: alpha(edge, 0.32),
            grid: alpha(ink, 0.07),
            player: alpha(ink, 0.12),
            ghost_throttle: mix(throttle, ink3, 0.55),
            ghost_brake: mix(brake, ink3, 0.55),
            ghost_attention: mix(attention, ink3, 0.70),
            ghost_wheel: mix(wheel, ink3, 0.55),
            f1_red: Color32::from_rgb(0xe1, 0x06, 0x00),
            radius,
            row_h,
        }
    }

    pub fn f1(&self) -> bool {
        self.id == ThemeId::F1
    }
    pub fn light(&self) -> bool {
        self.id == ThemeId::Light
    }

    pub fn font(&self, w: W, size: f32) -> FontId {
        let name = match (self.f1(), w) {
            (false, W::Medium) => BARLOW_500,
            (false, W::Semi) => BARLOW_600,
            (false, W::Bold | W::Heavy) => BARLOW_700,
            (true, W::Medium) => TITILLIUM_400,
            (true, W::Semi) => TITILLIUM_600,
            (true, W::Bold) => TITILLIUM_700,
            (true, W::Heavy) => TITILLIUM_900,
        };
        FontId::new(size, FontFamily::Name(name.into()))
    }
    pub fn medium(&self, size: f32) -> FontId {
        self.font(W::Medium, size)
    }
    pub fn semi(&self, size: f32) -> FontId {
        self.font(W::Semi, size)
    }
    pub fn bold(&self, size: f32) -> FontId {
        self.font(W::Bold, size)
    }
    pub fn heavy(&self, size: f32) -> FontId {
        self.font(W::Heavy, size)
    }

    /// Box Box writes names and headings in capitals.
    pub fn caps(&self, s: &str) -> String {
        if self.f1() {
            s.to_uppercase()
        } else {
            s.to_string()
        }
    }
}

const BARLOW_500: &str = "barlow-500";
const BARLOW_600: &str = "barlow-600";
const BARLOW_700: &str = "barlow-700";
const TITILLIUM_400: &str = "titillium-400";
const TITILLIUM_600: &str = "titillium-600";
const TITILLIUM_700: &str = "titillium-700";
const TITILLIUM_900: &str = "titillium-900";

/// The web app's fonts, bundled (SIL Open Font Licence): Barlow Semi
/// Condensed, and Titillium Web for Box Box (latin and latin-ext subsets).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    let family = |fonts: &mut FontDefinitions, name: &str, files: &[(&str, &'static [u8])]| {
        let mut chain = Vec::new();
        for (file, bytes) in files {
            fonts.font_data.insert((*file).to_owned(), Arc::new(FontData::from_static(bytes)));
            chain.push((*file).to_owned());
        }
        // egui's own fonts stay as fallbacks (arrows and symbols).
        chain.extend(fallbacks.iter().cloned());
        // Hack has the arrows and triangles (▲ ▼ ↑ ↓ ●) the web gets from system fonts.
        chain.push("Hack".to_owned());
        fonts.families.insert(FontFamily::Name(name.into()), chain);
    };
    family(&mut fonts, BARLOW_500, &[("barlow-500", include_bytes!("../assets/fonts/BarlowSemiCondensed-Medium.ttf"))]);
    family(&mut fonts, BARLOW_600, &[("barlow-600", include_bytes!("../assets/fonts/BarlowSemiCondensed-SemiBold.ttf"))]);
    family(&mut fonts, BARLOW_700, &[("barlow-700", include_bytes!("../assets/fonts/BarlowSemiCondensed-Bold.ttf"))]);
    family(
        &mut fonts,
        TITILLIUM_400,
        &[
            ("t400", include_bytes!("../assets/fonts/TitilliumWeb-400-latin.ttf")),
            ("t400x", include_bytes!("../assets/fonts/TitilliumWeb-400-latin-ext.ttf")),
        ],
    );
    family(
        &mut fonts,
        TITILLIUM_600,
        &[
            ("t600", include_bytes!("../assets/fonts/TitilliumWeb-600-latin.ttf")),
            ("t600x", include_bytes!("../assets/fonts/TitilliumWeb-600-latin-ext.ttf")),
        ],
    );
    family(
        &mut fonts,
        TITILLIUM_700,
        &[
            ("t700", include_bytes!("../assets/fonts/TitilliumWeb-700-latin.ttf")),
            ("t700x", include_bytes!("../assets/fonts/TitilliumWeb-700-latin-ext.ttf")),
        ],
    );
    family(
        &mut fonts,
        TITILLIUM_900,
        &[
            ("t900", include_bytes!("../assets/fonts/TitilliumWeb-900-latin.ttf")),
            ("t900x", include_bytes!("../assets/fonts/TitilliumWeb-900-latin-ext.ttf")),
        ],
    );
    // Plain egui text (the edit bar) uses Barlow too.
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "barlow-500".to_owned());
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn custom_colours_and_follows() {
        let mut own = HashMap::new();
        own.insert("gain".to_string(), "#00ff00".to_string());
        own.insert("ink".to_string(), "not a colour".to_string());
        let t = Theme::build(ThemeId::Dark, Some(&own));
        assert_eq!(t.gain, Color32::from_rgb(0, 255, 0));
        // Throttle follows gain until changed itself.
        assert_eq!(t.throttle, Color32::from_rgb(0, 255, 0));
        assert_eq!(t.ink, parse_hex("#e9eef3").unwrap());
        let l = Theme::build(ThemeId::Light, None);
        assert_eq!(l.attention, parse_hex("#d48300").unwrap());
        assert_eq!(Theme::build(ThemeId::F1, None).radius, 0.0);
    }

    #[test]
    fn hex_forms() {
        assert_eq!(parse_hex("#fff"), Some(Color32::WHITE));
        assert_eq!(parse_hex("ffda59"), Some(Color32::from_rgb(0xff, 0xda, 0x59)));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(alpha(Color32::WHITE, 0.5).a(), 128);
    }
}
