//! The widgets, each drawn at its own origin by a `Widget`, and the registry
//! that makes one per id in `schema::WIDGETS`.

use std::cell::RefCell;

use egui::{Painter, Pos2, Rect, Vec2};

use crate::config::{Config, General, Opts, Units};
use crate::data::Live;
use crate::kit::Thumb;
use crate::ratings::Ratings;
use crate::theme::Theme;

pub mod battle;
pub mod coach;
pub mod delta;
pub mod dynamics;
pub mod flags;
pub mod fuel;
pub mod inputs;
pub mod instruments;
pub mod laplog;
pub mod pitstop;
pub mod racepace;
pub mod radar;
pub mod relative;
pub mod session;
pub mod standings;
pub mod stints;
pub mod tires;
pub mod twitch;
pub mod weather;

/// Everything a widget draws from.
pub struct Ctx<'a> {
    pub th: &'a Theme,
    pub o: Opts<'a>,
    pub general: &'a General,
    /// The whole config, for widgets that read another's options.
    pub cfg: &'a Config,
    pub units: Units,
    pub live: &'a Live,
    pub ratings: &'a Ratings,
    pub editing: bool,
    /// Drawn in a window of its own (the default), whose size is the widget's: nothing may stick out of it.
    pub windowed: bool,
    /// Seconds since the app started, for flashing and fading.
    pub now: f64,
    pub ctx: &'a egui::Context,
    /// Parts that take clicks (thumbs): a widget adds them as it draws, in its
    /// own space; the overlay keeps the window click-through except over them.
    pub clicks: &'a RefCell<Vec<Click>>,
    /// The mouse in this widget's own space (as of the last frame), when it's over the overlay.
    pub pointer: Option<Pos2>,
}

/// A part of a widget that takes clicks even outside edit mode.
#[derive(Debug, Clone, PartialEq)]
pub struct Click {
    /// In the widget's own space (unscaled, from its origin).
    pub rect: Rect,
    pub action: ClickAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClickAction {
    /// Set (or clear) your rating of a driver.
    Rate { user_id: i64, name: String, rating: Option<Thumb> },
}

pub trait Widget: Send {
    /// Called every frame for every widget, shown or not, so histories (race
    /// pace, delta trace, weather trends) keep filling while it's hidden.
    fn observe(&mut self, _live: &Live, _now: f64) {}

    /// Draws at the origin, in CSS px, and returns the size used. A zero size
    /// draws nothing (a radar with no cars near, no flag out).
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2;

    /// How much of the size `draw` returned lies above and below the widget's own box (a strip
    /// that sticks out of it, drawn in the window), as of the last draw. The window grows by it.
    fn overhang(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// Told before each draw in a window of its own: how much room there is above the widget's box
    /// on its display, in the widget's own px. A widget that sticks out above can go below instead.
    fn set_room_above(&mut self, _px: f32) {}
}

pub fn make(id: &str) -> Box<dyn Widget> {
    match id {
        "relative" => Box::new(relative::Relative),
        "standings" => Box::new(standings::Standings),
        "session" => Box::new(session::Session),
        "inputs" => Box::<inputs::Inputs>::default(),
        "delta" => Box::<delta::Delta>::default(),
        "laplog" => Box::<laplog::LapLog>::default(),
        "coach" => Box::new(coach::Coach),
        "dynamics" => Box::<dynamics::Dynamics>::default(),
        "instruments" => Box::<instruments::Instruments>::default(),
        "fuel" => Box::new(fuel::Fuel),
        "radar" => Box::new(radar::Radar),
        "tires" => Box::new(tires::Tires),
        "pitstop" => Box::new(pitstop::PitStop),
        "flags" => Box::<flags::Flags>::default(),
        "weather" => Box::<weather::Weather>::default(),
        "racepace" => Box::<racepace::RacePace>::default(),
        "stints" => Box::new(stints::Stints),
        "battle" => Box::<battle::Battle>::default(),
        "twitch" => Box::<twitch::Twitch>::default(),
        _ => unreachable!("unknown widget {id}"),
    }
}
