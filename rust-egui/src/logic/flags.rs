//! The flag board's logic from `src/widgets/Flags.tsx`: which flag shows
//! (priority order), when green shows, and the dot pattern of every board.

use egui::Color32;

/// irsdk_Flags bits in SessionFlags.
pub mod f {
    pub const CHECKERED: u32 = 0x1;
    pub const WHITE: u32 = 0x2;
    pub const GREEN: u32 = 0x4;
    pub const YELLOW: u32 = 0x8;
    pub const RED: u32 = 0x10;
    pub const BLUE: u32 = 0x20;
    pub const DEBRIS: u32 = 0x40;
    pub const YELLOW_WAVING: u32 = 0x100;
    pub const ONE_LAP_TO_GREEN: u32 = 0x200;
    pub const CAUTION: u32 = 0x4000;
    pub const CAUTION_WAVING: u32 = 0x8000;
    pub const BLACK: u32 = 0x10000;
    pub const DISQUALIFY: u32 = 0x20000;
    pub const FURLED: u32 = 0x80000;
    pub const REPAIR: u32 = 0x100000;
    pub const START_GO: u32 = 0x8000_0000;
}
const NOT_GREEN: u32 = f::CAUTION | f::CAUTION_WAVING | f::YELLOW | f::YELLOW_WAVING | f::RED | f::CHECKERED;

/// Real flag colours, not the theme's: the board is a physical sign.
pub mod led {
    use egui::Color32;
    pub const YELLOW: Color32 = Color32::from_rgb(0xff, 0xd2, 0x1a);
    pub const BLUE: Color32 = Color32::from_rgb(0x2e, 0x86, 0xff);
    pub const GREEN: Color32 = Color32::from_rgb(0x22, 0xd3, 0x6b);
    pub const RED: Color32 = Color32::from_rgb(0xff, 0x2b, 0x2b);
    pub const WHITE: Color32 = Color32::from_rgb(0xf4, 0xf4, 0xef);
    pub const ORANGE: Color32 = Color32::from_rgb(0xff, 0x8a, 0x00);
}

#[derive(Clone, PartialEq, Debug)]
pub enum Pattern {
    Fill(Color32),
    Text { text: String, color: Color32, border: Color32 },
    Checker,
    Meatball,
    Debris,
    Warning,
    Off,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Board {
    pub pattern: Pattern,
    /// Waved flags flash.
    pub flash: bool,
    /// Option key for the caption under the board.
    pub text_key: &'static str,
    pub color: Color32,
}

fn board(pattern: Pattern, flash: bool, text_key: &'static str, color: Color32) -> Option<Board> {
    Some(Board { pattern, flash, text_key, color })
}

fn text(t: &str, color: Color32, border: Color32) -> Pattern {
    Pattern::Text { text: t.to_string(), color, border }
}

/// The flag to show, most serious first. `show_green` says the green window
/// is open; `race` gates "1 to green" (iRacing also sets it in practice).
pub fn board_for(flags: u32, show_green: bool, sc_text: &str, race: bool) -> Option<Board> {
    let has = |m: u32| flags & m != 0;
    if has(f::DISQUALIFY) {
        return board(text("DSQ", led::WHITE, led::RED), false, "textDisqualified", led::RED);
    }
    if has(f::BLACK) {
        return board(text("BLK", led::WHITE, led::WHITE), false, "textBlack", led::WHITE);
    }
    if has(f::REPAIR) {
        return board(Pattern::Meatball, false, "textRepair", led::ORANGE);
    }
    if has(f::FURLED) {
        return board(Pattern::Warning, false, "textWarning", led::WHITE);
    }
    if has(f::RED) {
        return board(Pattern::Fill(led::RED), false, "textRed", led::RED);
    }
    if has(f::CHECKERED) {
        return board(Pattern::Checker, false, "textCheckered", led::WHITE);
    }
    if has(f::CAUTION | f::CAUTION_WAVING) {
        return board(text(sc_text, led::WHITE, led::YELLOW), has(f::CAUTION_WAVING), "textCaution", led::YELLOW);
    }
    if has(f::YELLOW | f::YELLOW_WAVING) {
        return board(Pattern::Fill(led::YELLOW), has(f::YELLOW_WAVING), "textYellow", led::YELLOW);
    }
    if has(f::DEBRIS) {
        return board(Pattern::Debris, false, "textDebris", led::YELLOW);
    }
    if race && has(f::ONE_LAP_TO_GREEN) {
        return board(text("1", led::WHITE, led::GREEN), false, "textOneToGreen", led::GREEN);
    }
    if has(f::BLUE) {
        return board(Pattern::Fill(led::BLUE), true, "textBlue", led::BLUE);
    }
    if has(f::WHITE) {
        return board(Pattern::Fill(led::WHITE), false, "textWhite", led::WHITE);
    }
    if show_green {
        return board(Pattern::Fill(led::GREEN), false, "textGreen", led::GREEN);
    }
    None
}

/// The text board's letters: the option, upper-cased, at most three.
pub fn sc_text(option: &str) -> String {
    let s: String = option.to_uppercase().chars().take(3).collect();
    if s.is_empty() {
        "SC".into()
    } else {
        s
    }
}

pub fn is_green(flags: u32) -> bool {
    flags & (f::GREEN | f::START_GO) != 0 && flags & NOT_GREEN == 0
}

/// Green only shows for a moment after a start or restart, then the board goes dark.
#[derive(Default)]
pub struct GreenWindow {
    prev: Option<u32>,
    /// When green last came on (app seconds).
    started: Option<f64>,
}

impl GreenWindow {
    /// Feed it the flags each frame (None before any data). The first flags
    /// seen are the state of play, not a start.
    pub fn observe(&mut self, flags: Option<u32>, now: f64) {
        if let Some(fl) = flags {
            if self.prev.is_some_and(|p| is_green(fl) && !is_green(p)) {
                self.started = Some(now);
            }
            self.prev = Some(fl);
        }
    }

    pub fn open(&self, now: f64, seconds: f64) -> bool {
        self.started.is_some_and(|s| now < s + seconds)
    }
}

pub const DOTS: usize = 28;

/// The colour of every dot, row by row; None for unlit.
pub fn lit_dots(p: &Pattern) -> Vec<Option<Color32>> {
    let mut dots = vec![None; DOTS * DOTS];
    let mid = (DOTS as f32 - 1.0) / 2.0;
    let each = |dots: &mut Vec<Option<Color32>>, f: &dyn Fn(usize, usize) -> Option<Color32>| {
        for y in 0..DOTS {
            for x in 0..DOTS {
                dots[y * DOTS + x] = f(x, y);
            }
        }
    };
    match p {
        Pattern::Fill(c) => each(&mut dots, &|_, _| Some(*c)),
        Pattern::Checker => each(&mut dots, &|x, y| ((x / 4 + y / 4) % 2 == 0).then_some(led::WHITE)),
        Pattern::Meatball => each(&mut dots, &|x, y| ((x as f32 - mid).hypot(y as f32 - mid) <= 9.5).then_some(led::ORANGE)),
        // The surface flag: yellow with red stripes.
        Pattern::Debris => each(&mut dots, &|x, _| Some(if (x / 4) % 2 == 0 { led::YELLOW } else { led::RED })),
        // Black and white, split on the diagonal.
        Pattern::Warning => each(&mut dots, &|x, y| (x + y < DOTS - 1).then_some(led::WHITE)),
        Pattern::Text { text, color, border } => {
            each(&mut dots, &|x, y| (x.min(y).min(DOTS - 1 - x).min(DOTS - 1 - y) < 2).then_some(*border));
            let chars: Vec<&[&str; 7]> = text.chars().map(glyph).collect();
            let n = chars.len();
            if n > 0 {
                let scale = if n <= 2 { 2 } else { 1 };
                let w = n * 5 * scale + (n - 1) * scale;
                let h = 7 * scale;
                // JS Math.round of a half rounds up.
                let x0 = ((DOTS as f32 - w as f32) / 2.0 + 0.5).floor() as i32;
                let y0 = ((DOTS as f32 - h as f32) / 2.0 + 0.5).floor() as i32;
                for (i, g) in chars.iter().enumerate() {
                    for (ry, row) in g.iter().enumerate() {
                        for (rx, bit) in row.chars().enumerate() {
                            if bit != '1' {
                                continue;
                            }
                            for sy in 0..scale {
                                for sx in 0..scale {
                                    let px = x0 + (i * 6 * scale + rx * scale + sx) as i32;
                                    let py = y0 + (ry * scale + sy) as i32;
                                    if (0..DOTS as i32).contains(&px) && (0..DOTS as i32).contains(&py) {
                                        dots[py as usize * DOTS + px as usize] = Some(*color);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Pattern::Off => {}
    }
    dots
}

/// 5x7 dot-matrix font (a space for anything unknown).
fn glyph(c: char) -> &'static [&'static str; 7] {
    const fn g(a: &'static str, b: &'static str, c: &'static str, d: &'static str, e: &'static str, f: &'static str, h: &'static str) -> [&'static str; 7] {
        [a, b, c, d, e, f, h]
    }
    static SPACE: [&str; 7] = g("00000", "00000", "00000", "00000", "00000", "00000", "00000");
    static LETTERS: [[&str; 7]; 26] = [
        g("01110", "10001", "10001", "11111", "10001", "10001", "10001"),
        g("11110", "10001", "10001", "11110", "10001", "10001", "11110"),
        g("01110", "10001", "10000", "10000", "10000", "10001", "01110"),
        g("11110", "10001", "10001", "10001", "10001", "10001", "11110"),
        g("11111", "10000", "10000", "11110", "10000", "10000", "11111"),
        g("11111", "10000", "10000", "11110", "10000", "10000", "10000"),
        g("01110", "10001", "10000", "10111", "10001", "10001", "01111"),
        g("10001", "10001", "10001", "11111", "10001", "10001", "10001"),
        g("01110", "00100", "00100", "00100", "00100", "00100", "01110"),
        g("00111", "00010", "00010", "00010", "00010", "10010", "01100"),
        g("10001", "10010", "10100", "11000", "10100", "10010", "10001"),
        g("10000", "10000", "10000", "10000", "10000", "10000", "11111"),
        g("10001", "11011", "10101", "10101", "10001", "10001", "10001"),
        g("10001", "10001", "11001", "10101", "10011", "10001", "10001"),
        g("01110", "10001", "10001", "10001", "10001", "10001", "01110"),
        g("11110", "10001", "10001", "11110", "10000", "10000", "10000"),
        g("01110", "10001", "10001", "10001", "10101", "10010", "01101"),
        g("11110", "10001", "10001", "11110", "10100", "10010", "10001"),
        g("01111", "10000", "10000", "01110", "00001", "00001", "11110"),
        g("11111", "00100", "00100", "00100", "00100", "00100", "00100"),
        g("10001", "10001", "10001", "10001", "10001", "10001", "01110"),
        g("10001", "10001", "10001", "10001", "10001", "01010", "00100"),
        g("10001", "10001", "10001", "10101", "10101", "10101", "01010"),
        g("10001", "10001", "01010", "00100", "01010", "10001", "10001"),
        g("10001", "10001", "01010", "00100", "00100", "00100", "00100"),
        g("11111", "00001", "00010", "00100", "01000", "10000", "11111"),
    ];
    static DIGITS: [[&str; 7]; 10] = [
        g("01110", "10001", "10011", "10101", "11001", "10001", "01110"),
        g("00100", "01100", "00100", "00100", "00100", "00100", "01110"),
        g("01110", "10001", "00001", "00010", "00100", "01000", "11111"),
        g("11111", "00010", "00100", "00010", "00001", "10001", "01110"),
        g("00010", "00110", "01010", "10010", "11111", "00010", "00010"),
        g("11111", "10000", "11110", "00001", "00001", "10001", "01110"),
        g("00110", "01000", "10000", "11110", "10001", "10001", "01110"),
        g("11111", "00001", "00010", "00100", "01000", "01000", "01000"),
        g("01110", "10001", "10001", "01110", "10001", "10001", "01110"),
        g("01110", "10001", "10001", "01111", "00001", "00010", "01100"),
    ];
    static DASH: [&str; 7] = g("00000", "00000", "00000", "11111", "00000", "00000", "00000");
    match c {
        'A'..='Z' => &LETTERS[c as usize - 'A' as usize],
        '0'..='9' => &DIGITS[c as usize - '0' as usize],
        '-' => &DASH,
        _ => &SPACE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(b: Option<Board>) -> Option<&'static str> {
        b.map(|b| b.text_key)
    }

    #[test]
    fn priority_order() {
        let all = f::DISQUALIFY | f::BLACK | f::REPAIR | f::RED | f::YELLOW | f::BLUE;
        assert_eq!(kind(board_for(all, true, "SC", true)), Some("textDisqualified"));
        assert_eq!(kind(board_for(all & !f::DISQUALIFY, true, "SC", true)), Some("textBlack"));
        assert_eq!(kind(board_for(f::REPAIR | f::RED, false, "SC", true)), Some("textRepair"));
        assert_eq!(kind(board_for(f::RED | f::CHECKERED, false, "SC", true)), Some("textRed"));
        assert_eq!(kind(board_for(f::CAUTION | f::YELLOW, false, "SC", true)), Some("textCaution"));
        assert_eq!(kind(board_for(f::YELLOW | f::DEBRIS, false, "SC", true)), Some("textYellow"));
        assert_eq!(kind(board_for(f::BLUE | f::WHITE, false, "SC", true)), Some("textBlue"));
        assert_eq!(kind(board_for(f::WHITE, true, "SC", true)), Some("textWhite"));
    }

    #[test]
    fn waved_flags_flash() {
        assert!(!board_for(f::YELLOW, false, "SC", true).unwrap().flash);
        assert!(board_for(f::YELLOW_WAVING, false, "SC", true).unwrap().flash);
        assert!(board_for(f::CAUTION_WAVING, false, "SC", true).unwrap().flash);
        assert!(board_for(f::BLUE, false, "SC", true).unwrap().flash);
    }

    #[test]
    fn green_only_while_the_window_is_open() {
        assert_eq!(board_for(f::GREEN, false, "SC", true), None);
        assert_eq!(kind(board_for(f::GREEN, true, "SC", true)), Some("textGreen"));
        assert_eq!(board_for(0, false, "SC", true), None);
    }

    #[test]
    fn one_to_green_only_in_races() {
        assert_eq!(kind(board_for(f::ONE_LAP_TO_GREEN, false, "SC", true)), Some("textOneToGreen"));
        assert_eq!(board_for(f::ONE_LAP_TO_GREEN, false, "SC", false), None);
    }

    #[test]
    fn green_window_opens_on_a_start_not_on_the_first_flags() {
        let mut g = GreenWindow::default();
        g.observe(Some(f::GREEN), 1.0);
        assert!(!g.open(1.0, 5.0), "first flags seen are the state of play");
        g.observe(Some(f::YELLOW), 2.0);
        g.observe(Some(f::GREEN), 10.0);
        assert!(g.open(12.0, 5.0));
        assert!(!g.open(15.5, 5.0));
        // A green flag with a yellow still out is not green.
        g.observe(Some(f::GREEN | f::YELLOW), 30.0);
        assert!(!g.open(30.5, 5.0));
        g.observe(Some(f::GREEN | f::START_GO), 31.0);
        assert!(g.open(32.0, 5.0));
        g.observe(None, 40.0);
        g.observe(Some(f::START_GO), 41.0);
        g.observe(Some(0), 42.0);
        g.observe(Some(f::START_GO), 43.0);
        assert!(g.open(44.0, 5.0));
    }

    #[test]
    fn sc_text_is_three_capitals() {
        assert_eq!(sc_text("sc"), "SC");
        assert_eq!(sc_text("pace"), "PAC");
        assert_eq!(sc_text(""), "SC");
    }

    fn count(p: &Pattern) -> usize {
        lit_dots(p).iter().filter(|d| d.is_some()).count()
    }

    #[test]
    fn patterns_light_the_right_dots() {
        assert_eq!(count(&Pattern::Off), 0);
        assert_eq!(count(&Pattern::Fill(led::RED)), DOTS * DOTS);
        // 7x7 blocks of 4: 25 of 49 are lit.
        assert_eq!(count(&Pattern::Checker), 25 * 16);
        assert_eq!(count(&Pattern::Debris), DOTS * DOTS);
        // Diagonal split: x + y < 27 is 378 dots.
        assert_eq!(count(&Pattern::Warning), 378);
        let ball = lit_dots(&Pattern::Meatball);
        assert!(ball[14 * DOTS + 14].is_some() && ball[0].is_none());
    }

    #[test]
    fn text_board_has_a_border_and_centred_text() {
        let dots = lit_dots(&text("1", led::WHITE, led::GREEN));
        // The border is two dots thick.
        assert_eq!(dots[0], Some(led::GREEN));
        assert_eq!(dots[DOTS + 1], Some(led::GREEN));
        assert_eq!(dots[2 * DOTS + 2], None);
        // A doubled 5x7 glyph: 10 wide, 14 tall, centred at (9..19, 7..21).
        let white: Vec<usize> = (0..DOTS * DOTS).filter(|i| dots[*i] == Some(led::WHITE)).collect();
        assert!(!white.is_empty());
        assert!(white.iter().all(|i| (7..21).contains(&(i / DOTS)) && (9..19).contains(&(i % DOTS))));
    }

    #[test]
    fn every_glyph_is_five_by_seven() {
        for c in ('A'..='Z').chain('0'..='9').chain(['-', ' ']) {
            let g = glyph(c);
            assert!(g.iter().all(|r| r.len() == 5), "{c}");
        }
    }
}
