//! The connected displays and where the overlay windows go
//! (`src-tauri/src/displays.rs`, `src/lib/displays.ts`).
//!
//! The overlay is one transparent window per display. Each shows the widgets
//! placed on its display (`widget.display` in config.json, the display's key),
//! in that display's own CSS pixels (points here), so mixed-DPI setups land
//! widgets in the right place.
//!
//! Display keys are the Tauri app's: Windows' device name (`\\.\DISPLAY2`),
//! else the display's position (`x,y`). The list is primary first, then left
//! to right, top to bottom, numbered from 1.
//!
//! Windows reads the list with `EnumDisplayMonitors`; elsewhere there is no
//! list (the app makes one display from the window's monitor), except for
//! testing: `TENTHS_EGUI_DISPLAYS="0,0,1920,1080;1920,0,2560,1440,1.5"` fakes
//! displays (x,y,width,height[,scale] in physical pixels, primary first), so
//! several overlay windows can be tried on one big xvfb screen.

use egui::{Pos2, Rect, Vec2};

/// A display as the overlay sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    /// Stable id: Windows' device name (e.g. `\\.\DISPLAY2`), else its position.
    pub key: String,
    /// 1 for the primary, then left to right, top to bottom.
    pub number: u32,
    pub primary: bool,
    /// Physical pixels on the virtual desktop.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Windows display scaling: 1.0 = 100%, 1.5 = 150%.
    pub scale: f64,
}

impl Display {
    /// "Display 2 · 2560×1440", or "Display 1 (main) · …" for the primary.
    pub fn name(&self) -> String {
        format!("Display {}{} · {}×{}", self.number, if self.primary { " (main)" } else { "" }, self.width, self.height)
    }

    /// The size in CSS px (points): what its overlay window's content is.
    pub fn css_size(&self) -> Vec2 {
        Vec2::new((self.width as f64 / self.scale) as f32, (self.height as f64 / self.scale) as f32)
    }
}

/// The key for a display with this device name, else its position.
pub fn key_of(name: Option<&str>, x: i32, y: i32) -> String {
    match name {
        Some(n) if !n.trim().is_empty() => n.to_string(),
        _ => format!("{x},{y}"),
    }
}

/// Primary first, then left to right and top to bottom; one per key;
/// numbered from 1. With no primary reported, the first is the primary.
pub fn finish(mut list: Vec<Display>) -> Vec<Display> {
    if !list.iter().any(|d| d.primary) {
        if let Some(d) = list.first_mut() {
            d.primary = true;
        }
    }
    list.sort_by_key(|d| (!d.primary, d.x, d.y));
    let mut seen = std::collections::HashSet::new();
    list.retain(|d| seen.insert(d.key.clone()));
    for (i, d) in list.iter_mut().enumerate() {
        d.number = i as u32 + 1;
    }
    list
}

pub fn primary_key(displays: &[Display]) -> Option<&str> {
    displays.iter().find(|d| d.primary).or(displays.first()).map(|d| d.key.as_str())
}

/// The display a widget shows on: the one it was placed on, or the primary
/// display when that one isn't connected (or the widget was never placed).
pub fn widget_display<'a>(display: Option<&str>, displays: &'a [Display]) -> Option<&'a str> {
    if let Some(d) = display.and_then(|k| displays.iter().find(|d| d.key == k)) {
        return Some(d.key.as_str());
    }
    primary_key(displays)
}

/// The display after this one, wrapping round: for the edit label's display button.
pub fn next_display<'a>(key: &str, displays: &'a [Display]) -> Option<&'a Display> {
    if displays.len() < 2 {
        return None;
    }
    let i = displays.iter().position(|d| d.key == key).unwrap_or(displays.len() - 1);
    Some(&displays[(i + 1) % displays.len()])
}

/// Which display a point on the desktop (physical pixels) is on, if any.
pub fn display_at(px: f64, py: f64, displays: &[Display]) -> Option<&Display> {
    displays.iter().find(|d| px >= d.x as f64 && px < d.x as f64 + d.width as f64 && py >= d.y as f64 && py < d.y as f64 + d.height as f64)
}

/// Where a widget dropped on another display goes, in that display's CSS px.
#[derive(Debug, Clone, PartialEq)]
pub struct Dropped<'a> {
    pub display: &'a Display,
    pub left: f32,
    pub top: f32,
}

/// `from` is the display being dragged on; `client` is the pointer in its
/// window (outside it, past the edge); `grab` is where the widget was held,
/// from its top-left corner, in CSS px. The widget keeps its CSS size, so the
/// same offset applies on the other side.
pub fn drop_onto<'a>(from: &Display, client: Pos2, grab: Vec2, displays: &'a [Display]) -> Option<Dropped<'a>> {
    let px = from.x as f64 + client.x as f64 * from.scale;
    let py = from.y as f64 + client.y as f64 * from.scale;
    let to = display_at(px, py, displays)?;
    if to.key == from.key {
        return None;
    }
    Some(Dropped { display: to, left: ((px - to.x as f64) / to.scale) as f32 - grab.x, top: ((py - to.y as f64) / to.scale) as f32 - grab.y })
}

/// A desktop point (physical px) as a position in this display's window.
pub fn to_client(from: &Display, px: f64, py: f64) -> Pos2 {
    Pos2::new(((px - from.x as f64) / from.scale) as f32, ((py - from.y as f64) / from.scale) as f32)
}

// --- Placing the windows --------------------------------------------------------

/// What to ask a window for so it covers `d`, in points, when its own scale
/// is `ppp` (winit turns points into pixels with the scale of the monitor the
/// window is on *now*, so this is asked again once the window has moved).
pub fn window_rect(d: &Display, ppp: f32) -> Rect {
    Rect::from_min_size(
        Pos2::new(d.x as f32 / ppp, d.y as f32 / ppp),
        Vec2::new(d.width as f32 / ppp, d.height as f32 / ppp),
    )
}

/// Does a window whose inner area is `inner` (points) at scale `ppp` cover `d`?
pub fn covers(d: &Display, inner: Rect, ppp: f32) -> bool {
    let want = [d.x as f32, d.y as f32, d.width as f32, d.height as f32];
    let have = [inner.min.x * ppp, inner.min.y * ppp, inner.width() * ppp, inner.height() * ppp];
    want.iter().zip(have).all(|(w, h)| (w - h).abs() <= 1.5)
}

/// Moves and sizes one overlay window onto its display, asking again (a few
/// times) until it's there: the first request can be turned into the wrong
/// pixels when the window starts on a monitor with another scale.
#[derive(Default)]
pub struct Placer {
    target: Option<Display>,
    tries: u32,
    last: f64,
}

const PLACE_TRIES: u32 = 12;
const PLACE_EVERY: f64 = 0.25;

impl Placer {
    /// Call every frame from inside the window. Returns false while it's
    /// still being moved (so the caller can keep repainting).
    pub fn update(&mut self, ctx: &egui::Context, d: &Display, now: f64) -> bool {
        if self.target.as_ref() != Some(d) {
            self.target = Some(d.clone());
            self.tries = 0;
            self.last = f64::NEG_INFINITY;
        }
        let (inner, ppp) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().native_pixels_per_point));
        let ppp = ppp.unwrap_or(1.0);
        if inner.is_some_and(|r| covers(d, r, ppp)) {
            self.tries = 0;
            return true;
        }
        if self.tries < PLACE_TRIES && now - self.last >= PLACE_EVERY {
            self.tries += 1;
            self.last = now;
            let r = window_rect(d, ppp);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(r.size()));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(r.min));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        false
    }
}

// --- Listing the displays --------------------------------------------------------

/// The connected displays (empty where there's no way to ask).
pub fn current() -> Vec<Display> {
    #[cfg(windows)]
    {
        finish(win::enumerate())
    }
    #[cfg(not(windows))]
    {
        std::env::var("TENTHS_EGUI_DISPLAYS").ok().map(|s| parse_fake(&s)).unwrap_or_default()
    }
}

/// `x,y,w,h[,scale];…` as displays named like Windows' (`\\.\DISPLAY1`…).
pub fn parse_fake(s: &str) -> Vec<Display> {
    let list = s
        .split(';')
        .filter_map(|part| {
            let v: Vec<f64> = part.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            if v.len() < 4 {
                return None;
            }
            Some((v[0] as i32, v[1] as i32, v[2] as u32, v[3] as u32, v.get(4).copied().filter(|s| *s > 0.0).unwrap_or(1.0)))
        })
        .enumerate()
        .map(|(i, (x, y, width, height, scale))| Display {
            key: format!("\\\\.\\DISPLAY{}", i + 1),
            number: 0,
            primary: i == 0,
            x,
            y,
            width,
            height,
            scale,
        })
        .collect();
    finish(list)
}

/// Where the mouse is on the desktop (physical pixels) and whether its main
/// button is held. While a widget is dragged, this lets it follow the pointer
/// onto another display (the window may stop getting pointer events once the
/// mouse leaves it), and it finds the thumbs under the pointer while the
/// overlay is click-through. None where it can't be read (not Windows).
#[derive(Debug, Clone, Copy)]
pub struct Pointer {
    pub x: f64,
    pub y: f64,
    pub down: bool,
}

pub fn pointer() -> Option<Pointer> {
    #[cfg(windows)]
    {
        win::pointer()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Stops the overlay window titled `title` taking the keyboard when clicked
/// (`WS_EX_NOACTIVATE`), so a click on a thumb doesn't pull focus out of
/// iRacing. Returns false if the window wasn't found yet. A no-op elsewhere.
pub fn set_no_activate(title: &str, on: bool) -> bool {
    #[cfg(windows)]
    {
        win::set_no_activate(title, on)
    }
    #[cfg(not(windows))]
    {
        let _ = (title, on);
        true
    }
}

#[cfg(windows)]
mod win {
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, MONITORINFOF_PRIMARY, SM_SWAPBUTTON, WS_EX_NOACTIVATE,
    };

    use super::{key_of, Display};

    unsafe extern "system" fn each(monitor: HMONITOR, _dc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
        let out = &mut *(data as *mut Vec<Display>);
        let mut info: MONITORINFOEXW = zeroed();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) != 0 {
            let r = info.monitorInfo.rcMonitor;
            let len = info.szDevice.iter().position(|c| *c == 0).unwrap_or(info.szDevice.len());
            let name = String::from_utf16_lossy(&info.szDevice[..len]);
            let (mut dx, mut dy) = (96u32, 96u32);
            let scale = if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) == 0 && dx > 0 { dx as f64 / 96.0 } else { 1.0 };
            out.push(Display {
                key: key_of(Some(&name), r.left, r.top),
                number: 0,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                x: r.left,
                y: r.top,
                width: (r.right - r.left).max(0) as u32,
                height: (r.bottom - r.top).max(0) as u32,
                scale,
            });
        }
        1
    }

    pub fn enumerate() -> Vec<Display> {
        let mut out: Vec<Display> = Vec::new();
        unsafe {
            EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(each), &mut out as *mut Vec<Display> as LPARAM);
        }
        out
    }

    pub fn pointer() -> Option<super::Pointer> {
        unsafe {
            let mut p = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut p) == 0 {
                return None;
            }
            // GetAsyncKeyState reads the physical buttons, so follow "switch primary and secondary buttons".
            let swapped = GetSystemMetrics(SM_SWAPBUTTON) != 0;
            let vk = if swapped { VK_RBUTTON } else { VK_LBUTTON };
            // The high bit is set while the button is down.
            Some(super::Pointer { x: p.x as f64, y: p.y as f64, down: GetAsyncKeyState(vk as i32) < 0 })
        }
    }

    pub fn set_no_activate(title: &str, on: bool) -> bool {
        let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let hwnd = FindWindowW(std::ptr::null(), wide.as_ptr());
            if hwnd.is_null() {
                return false;
            }
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let flag = WS_EX_NOACTIVATE as isize;
            let next = if on { style | flag } else { style & !flag };
            if next != style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(key: &str, primary: bool, x: i32, y: i32, w: u32, h: u32, scale: f64) -> Display {
        Display { key: key.into(), number: 0, primary, x, y, width: w, height: h, scale }
    }

    fn two() -> Vec<Display> {
        // A 1440p display on the left at 100%, the primary 4K on its right at 150%.
        finish(vec![d("\\\\.\\DISPLAY2", false, -2560, 0, 2560, 1440, 1.0), d("\\\\.\\DISPLAY1", true, 0, 0, 3840, 2160, 1.5)])
    }

    #[test]
    fn keys_numbers_and_order() {
        assert_eq!(key_of(Some("\\\\.\\DISPLAY2"), 5, 6), "\\\\.\\DISPLAY2");
        assert_eq!(key_of(Some("  "), 5, 6), "5,6");
        assert_eq!(key_of(None, -1920, 0), "-1920,0");
        let l = two();
        assert_eq!(l.iter().map(|d| (d.key.as_str(), d.number)).collect::<Vec<_>>(), [("\\\\.\\DISPLAY1", 1), ("\\\\.\\DISPLAY2", 2)]);
        // No primary reported: the first is it. Duplicates by key dropped.
        let l = finish(vec![d("a", false, 100, 0, 10, 10, 1.0), d("b", false, 0, 0, 10, 10, 1.0), d("a", false, 100, 0, 10, 10, 1.0)]);
        assert_eq!(l.len(), 2);
        assert!(l[0].primary && l[0].key == "a");
        assert_eq!(l[1].number, 2);
        // Left to right then top to bottom after the primary.
        let l = finish(vec![d("p", true, 0, 0, 10, 10, 1.0), d("c", false, 20, 0, 10, 10, 1.0), d("b", false, 10, 5, 10, 10, 1.0), d("a", false, 10, 0, 10, 10, 1.0)]);
        assert_eq!(l.iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["p", "a", "b", "c"]);
    }

    #[test]
    fn names_and_css_size() {
        let l = two();
        assert_eq!(l[0].name(), "Display 1 (main) · 3840×2160");
        assert_eq!(l[1].name(), "Display 2 · 2560×1440");
        assert_eq!(l[0].css_size(), Vec2::new(2560.0, 1440.0));
    }

    #[test]
    fn widgets_fall_back_to_the_primary() {
        let l = two();
        assert_eq!(widget_display(Some("\\\\.\\DISPLAY2"), &l), Some("\\\\.\\DISPLAY2"));
        assert_eq!(widget_display(Some("\\\\.\\DISPLAY9"), &l), Some("\\\\.\\DISPLAY1"));
        assert_eq!(widget_display(None, &l), Some("\\\\.\\DISPLAY1"));
        assert_eq!(widget_display(None, &[]), None);
    }

    #[test]
    fn next_display_wraps() {
        let l = two();
        assert_eq!(next_display("\\\\.\\DISPLAY1", &l).unwrap().number, 2);
        assert_eq!(next_display("\\\\.\\DISPLAY2", &l).unwrap().number, 1);
        assert!(next_display("x", &l[..1]).is_none());
    }

    #[test]
    fn display_at_edges() {
        let l = two();
        assert_eq!(display_at(0.0, 0.0, &l).unwrap().number, 1);
        assert_eq!(display_at(-1.0, 10.0, &l).unwrap().number, 2);
        assert_eq!(display_at(-2560.0, 0.0, &l).unwrap().number, 2);
        assert!(display_at(-2561.0, 0.0, &l).is_none());
        assert!(display_at(3840.0, 0.0, &l).is_none());
    }

    #[test]
    fn drop_keeps_the_grab_point_across_scales() {
        let l = two();
        let main = &l[0];
        // Dragging on the main display (2560×1440 CSS px) past its left edge: 100 CSS px
        // outside = 150 physical px, over the left display at x = -150 -> 2410 px in it.
        let grab = Vec2::new(30.0, 10.0);
        let drop = drop_onto(main, Pos2::new(-100.0, 200.0), grab, &l).unwrap();
        assert_eq!(drop.display.number, 2);
        assert_eq!(drop.left, 2410.0 - 30.0);
        assert_eq!(drop.top, 300.0 - 10.0);
        // And back the other way: from the 100% display onto the 150% one.
        let left = &l[1];
        let drop = drop_onto(left, Pos2::new(2560.0 + 90.0, 60.0), grab, &l).unwrap();
        assert_eq!(drop.display.number, 1);
        assert_eq!(drop.left, 90.0 / 1.5 - 30.0);
        assert_eq!(drop.top, 60.0 / 1.5 - 10.0);
        // Inside its own window, or over no display: nowhere to go.
        assert!(drop_onto(main, Pos2::new(100.0, 100.0), grab, &l).is_none());
        assert!(drop_onto(main, Pos2::new(-5000.0, 100.0), grab, &l).is_none());
    }

    #[test]
    fn desktop_point_to_client() {
        let l = two();
        assert_eq!(to_client(&l[0], 300.0, 150.0), Pos2::new(200.0, 100.0));
        assert_eq!(to_client(&l[1], -2460.0, 10.0), Pos2::new(100.0, 10.0));
    }

    #[test]
    fn windows_are_placed_by_the_scale_they_are_on() {
        let l = two();
        let main = &l[0];
        // On a 150% monitor the window covers 2560×1440 points...
        let r = window_rect(main, 1.5);
        assert_eq!((r.min, r.size()), (Pos2::ZERO, Vec2::new(2560.0, 1440.0)));
        assert!(covers(main, r, 1.5));
        // ...but if it still has a 100% scale, the same pixels are 3840×2160 points.
        let r = window_rect(main, 1.0);
        assert_eq!(r.size(), Vec2::new(3840.0, 2160.0));
        assert!(covers(main, r, 1.0));
        assert!(!covers(main, r, 1.5));
        // A display left of the origin.
        let r = window_rect(&l[1], 1.0);
        assert_eq!(r.min, Pos2::new(-2560.0, 0.0));
        // Off by a pixel or two is fine, more isn't.
        assert!(covers(main, Rect::from_min_size(Pos2::new(1.0, 0.0), Vec2::new(2560.0, 1440.0)), 1.5));
        assert!(!covers(main, Rect::from_min_size(Pos2::new(10.0, 0.0), Vec2::new(2560.0, 1440.0)), 1.5));
    }

    #[test]
    fn fake_displays_are_named_like_windows() {
        let l = parse_fake("0,0,1920,1080; 1920,0,2560,1440,1.5;junk;5,5");
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].key.as_str(), l[0].primary, l[0].number), ("\\\\.\\DISPLAY1", true, 1));
        assert_eq!((l[1].key.as_str(), l[1].x, l[1].scale), ("\\\\.\\DISPLAY2", 1920, 1.5));
        assert!(parse_fake("").is_empty());
    }
}
