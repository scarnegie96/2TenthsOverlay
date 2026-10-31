//! Our own reader for the iRacing SDK shared memory (Windows only).
//!
//! iRacing publishes a memory-mapped file `Local\IRSDKMemMapFileName` laid out as:
//!
//! ```text
//! irsdk_header (112 bytes)
//!   ver, status, tickRate, sessionInfoUpdate, sessionInfoLen, sessionInfoOffset,
//!   numVars, varHeaderOffset, numBuf, bufLen, pad[2],
//!   varBuf[4] { tickCount, bufOffset, pad[2] }
//! session info YAML           (at sessionInfoOffset, sessionInfoLen bytes)
//! var headers[numVars]        (at varHeaderOffset, 144 bytes each)
//! telemetry buffers[numBuf]   (rotating, bufLen bytes each)
//! ```
//!
//! and signals `Local\IRSDKDataValidEvent` every time a new buffer is written (60 Hz).
//! We wait on that event, pick the buffer with the highest tick count, copy it out,
//! and check the tick didn't change while copying (iRacing may overwrite it).

use std::cell::Cell;
use std::collections::HashMap;
use std::{mem, ptr};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ, MEMORY_BASIC_INFORMATION,
    MEMORY_MAPPED_VIEW_ADDRESS,
};
use windows_sys::Win32::System::Threading::{OpenEventW, WaitForSingleObject, SYNCHRONIZATION_SYNCHRONIZE};
use windows_sys::Win32::UI::WindowsAndMessaging::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};

use crate::model::{Deltas, RawFrame, MAX_CARS};
use crate::pit::{PitCommand, PitService, BROADCAST_PIT_COMMAND};
use crate::shm::{self, VarLayout};
use crate::vars::{self, VarHeader, VAR_HEADER_SIZE};

const MEM_MAP_NAME: &str = "Local\\IRSDKMemMapFileName";
const DATA_VALID_EVENT_NAME: &str = "Local\\IRSDKDataValidEvent";
const BROADCAST_MSG_NAME: &str = "IRSDK_BROADCASTMSG";
const PIT_PRESSURE_VARS: [&str; 4] = ["PitSvLFP", "PitSvRFP", "PitSvLRP", "PitSvRRP"];
const STATUS_CONNECTED: i32 = 1;
const MAX_BUFS: usize = 4;

// Tire variables in LF, RF, LR, RR order; left / middle / right.
const TIRE_TEMP_VARS: [[&str; 3]; 4] = [
    ["LFtempCL", "LFtempCM", "LFtempCR"],
    ["RFtempCL", "RFtempCM", "RFtempCR"],
    ["LRtempCL", "LRtempCM", "LRtempCR"],
    ["RRtempCL", "RRtempCM", "RRtempCR"],
];
const TIRE_WEAR_VARS: [[&str; 3]; 4] = [
    ["LFwearL", "LFwearM", "LFwearR"],
    ["RFwearL", "RFwearM", "RFwearR"],
    ["LRwearL", "LRwearM", "LRwearR"],
    ["RRwearL", "RRwearM", "RRwearR"],
];
const TIRE_PRESSURE_VARS: [&str; 4] = ["LFcoldPressure", "RFcoldPressure", "LRcoldPressure", "RRcoldPressure"];

// Header field byte offsets.
const H_STATUS: usize = 4;
const H_SESSION_INFO_UPDATE: usize = 12;
const H_SESSION_INFO_LEN: usize = 16;
const H_SESSION_INFO_OFFSET: usize = 20;
const H_NUM_VARS: usize = 24;
const H_VAR_HEADER_OFFSET: usize = 28;
const H_NUM_BUF: usize = 32;
const H_BUF_LEN: usize = 36;
const H_VAR_BUF: usize = 48; // 4 x 16 bytes

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Sends a pit command to iRacing the way the SDK's `irsdk_broadcastMsg` does:
/// a broadcast window message with the message type and command packed into
/// wParam and the parameter in lParam. Doesn't wait, and iRacing never
/// replies; the `PitSv*` variables show whether it took.
pub fn send_pit_command(cmd: PitCommand) -> bool {
    let (mode, param) = cmd.encode();
    let wparam = (BROADCAST_PIT_COMMAND as u32 | (mode as u32) << 16) as usize;
    unsafe {
        let msg = RegisterWindowMessageW(wide(BROADCAST_MSG_NAME).as_ptr());
        msg != 0 && SendNotifyMessageW(HWND_BROADCAST, msg, wparam, param as isize) != 0
    }
}

/// Connection to the shared memory. Dropping it unmaps and closes the handles.
pub struct Irsdk {
    map: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    /// Size of the mapped view in bytes. Every read is checked against it.
    view_len: usize,
    /// Set once we've logged a bad read, so we don't spam the log.
    warned: Cell<bool>,
    event: HANDLE,
    vars: HashMap<String, VarHeader>,
    /// Layout the var headers were read from; None when we have none.
    var_layout: Option<VarLayout>,
    last_tick: i32,
    last_session_update: i32,
    buf: Vec<u8>,
}

// The raw pointers are only used from the owning thread.
unsafe impl Send for Irsdk {}

impl Irsdk {
    /// Opens the memory map. Returns None when iRacing isn't running.
    pub fn open() -> Option<Self> {
        unsafe {
            let map = OpenFileMappingW(FILE_MAP_READ, 0, wide(MEM_MAP_NAME).as_ptr());
            if map.is_null() {
                return None;
            }
            let view = MapViewOfFile(map, FILE_MAP_READ, 0, 0, 0);
            if view.Value.is_null() {
                CloseHandle(map);
                return None;
            }
            // Ask how big the view is, so header offsets can be checked.
            let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();
            let got = VirtualQuery(view.Value, &mut info, mem::size_of::<MEMORY_BASIC_INFORMATION>());
            let view_len = if got == 0 { 0 } else { info.RegionSize };
            if view_len < shm::HEADER_LEN {
                eprintln!("[telemetry] iRacing shared memory too small ({view_len} bytes)");
                UnmapViewOfFile(view);
                CloseHandle(map);
                return None;
            }
            let event = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, wide(DATA_VALID_EVENT_NAME).as_ptr());
            if event.is_null() {
                UnmapViewOfFile(view);
                CloseHandle(map);
                return None;
            }
            Some(Self {
                map,
                view,
                view_len,
                warned: Cell::new(false),
                event,
                vars: HashMap::new(),
                var_layout: None,
                last_tick: -1,
                last_session_update: -1,
                buf: Vec::new(),
            })
        }
    }

    fn base(&self) -> *const u8 {
        self.view.Value as *const u8
    }

    /// Start of `len` bytes at `offset` in the view, or None if they don't
    /// fit. Logs the first bad read only.
    fn checked(&self, offset: i64, len: i64) -> Option<usize> {
        let start = shm::checked_range(offset, len, self.view_len);
        if start.is_none() && !self.warned.replace(true) {
            eprintln!(
                "[telemetry] skipped a shared memory read out of range (offset {offset}, len {len}, view {})",
                self.view_len
            );
        }
        start
    }

    /// Copies `len` bytes at `offset` into `out`. Returns false, and leaves
    /// `out` alone, if they don't fit in the view.
    fn copy_into(&self, offset: i64, len: i64, out: &mut Vec<u8>) -> bool {
        let Some(start) = self.checked(offset, len) else { return false };
        let len = len as usize; // checked above: 0..=MAX_READ_LEN
        out.resize(len, 0);
        unsafe { ptr::copy_nonoverlapping(self.base().add(start), out.as_mut_ptr(), len) };
        true
    }

    /// Reads a header field. Returns 0 if it's outside the view (it can't be,
    /// as open() checks the view holds the whole header).
    fn header_i32(&self, offset: usize) -> i32 {
        match self.checked(offset as i64, 4) {
            Some(start) => unsafe { ptr::read_volatile(self.base().add(start) as *const i32) },
            None => 0,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.header_i32(H_STATUS) & STATUS_CONNECTED != 0
    }

    /// Blocks up to `timeout_ms` for a new frame. Returns true when a fresh
    /// buffer has been copied and can be read with the accessors below.
    pub fn wait_for_data(&mut self, timeout_ms: u32) -> bool {
        let signalled = unsafe { WaitForSingleObject(self.event, timeout_ms) } == WAIT_OBJECT_0;
        if !self.is_connected() {
            // The next session may use a different layout, so forget this one.
            self.last_tick = -1;
            self.last_session_update = -1;
            self.var_layout = None;
            self.vars.clear();
            self.buf.clear();
            return false;
        }
        let layout = self.var_layout_now();
        if shm::needs_var_reload(self.var_layout, layout) {
            self.load_var_headers(layout);
        }
        // Even without the signal a new buffer may have landed; checking is cheap.
        let _ = signalled;
        self.copy_latest_buffer()
    }

    fn var_layout_now(&self) -> VarLayout {
        VarLayout {
            num_vars: self.header_i32(H_NUM_VARS),
            var_header_offset: self.header_i32(H_VAR_HEADER_OFFSET),
            buf_len: self.header_i32(H_BUF_LEN),
        }
    }

    fn load_var_headers(&mut self, layout: VarLayout) {
        let len = layout.num_vars as i64 * VAR_HEADER_SIZE as i64;
        let mut raw = Vec::new();
        if !self.copy_into(layout.var_header_offset as i64, len, &mut raw) {
            // Bad layout: drop the old list and try again next frame.
            self.vars.clear();
            self.var_layout = None;
            return;
        }
        self.vars = vars::parse_headers(&raw, layout.num_vars as usize);
        self.var_layout = Some(layout);
    }

    fn copy_latest_buffer(&mut self) -> bool {
        let num_buf = (self.header_i32(H_NUM_BUF).max(1) as usize).min(MAX_BUFS);
        let buf_len = self.header_i32(H_BUF_LEN);

        for _attempt in 0..3 {
            let (mut best_tick, mut best_off) = (i32::MIN, 0i32);
            for i in 0..num_buf {
                let tick = self.header_i32(H_VAR_BUF + i * 16);
                if tick > best_tick {
                    best_tick = tick;
                    best_off = self.header_i32(H_VAR_BUF + i * 16 + 4);
                }
            }
            if best_tick == self.last_tick {
                return false;
            }
            let mut buf = mem::take(&mut self.buf);
            let copied = self.copy_into(best_off as i64, buf_len as i64, &mut buf);
            self.buf = buf;
            if !copied {
                return false;
            }

            // Verify iRacing didn't recycle the buffer mid-copy.
            let still = (0..num_buf).any(|i| {
                self.header_i32(H_VAR_BUF + i * 16) == best_tick
                    && self.header_i32(H_VAR_BUF + i * 16 + 4) == best_off
            });
            if still {
                self.last_tick = best_tick;
                return true;
            }
        }
        false
    }

    pub fn tick(&self) -> i32 {
        self.last_tick
    }

    /// Returns the session YAML when it changed since the last call.
    pub fn session_yaml_if_changed(&mut self) -> Option<String> {
        let update = self.header_i32(H_SESSION_INFO_UPDATE);
        if update == self.last_session_update {
            return None;
        }
        let len = self.header_i32(H_SESSION_INFO_LEN);
        let off = self.header_i32(H_SESSION_INFO_OFFSET);
        let mut bytes = Vec::new();
        if !self.copy_into(off as i64, len as i64, &mut bytes) {
            return None;
        }
        // Discard if it changed while we were copying; we'll catch it next frame.
        if self.header_i32(H_SESSION_INFO_UPDATE) != update {
            return None;
        }
        self.last_session_update = update;
        Some(crate::session::decode_latin1(&bytes))
    }

    pub fn var(&self, name: &str) -> Option<&VarHeader> {
        self.vars.get(name)
    }

    pub fn vars(&self) -> impl Iterator<Item = &VarHeader> {
        self.vars.values()
    }

    /// Reads element `idx` of a variable as f64 regardless of its declared type.
    pub fn num(&self, name: &str, idx: usize) -> Option<f64> {
        vars::read_num(&self.buf, self.vars.get(name)?, idx)
    }

    pub fn f32(&self, name: &str) -> f32 {
        self.num(name, 0).unwrap_or(0.0) as f32
    }
    pub fn f64(&self, name: &str) -> f64 {
        self.num(name, 0).unwrap_or(0.0)
    }
    pub fn i32(&self, name: &str) -> i32 {
        self.num(name, 0).unwrap_or(0.0) as i32
    }
    pub fn bool(&self, name: &str) -> bool {
        self.num(name, 0).unwrap_or(0.0) != 0.0
    }

    fn fill<T: Copy>(&self, name: &str, out: &mut [T], conv: impl Fn(f64) -> T) {
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(v) = self.num(name, i) {
                *slot = conv(v);
            }
        }
    }

    /// Maps the variables we use into a normalized frame.
    pub fn read_frame(&self, f: &mut RawFrame) {
        f.tick = self.last_tick;
        f.session_num = self.i32("SessionNum");
        f.session_state = self.i32("SessionState");
        f.session_flags = self.num("SessionFlags", 0).unwrap_or(0.0) as u32;
        f.session_time = self.f64("SessionTime");
        f.session_time_remain = self.f64("SessionTimeRemain");
        f.session_laps_remain = self.i32("SessionLapsRemainEx");

        f.player_car_idx = self.i32("PlayerCarIdx");
        f.is_on_track = self.bool("IsOnTrack");
        f.on_pit_road = self.bool("OnPitRoad");

        f.speed = self.f32("Speed");
        f.rpm = self.f32("RPM");
        f.gear = self.i32("Gear");
        f.throttle = self.f32("Throttle");
        f.brake = self.f32("Brake");
        f.clutch = 1.0 - self.f32("Clutch");
        f.steering = self.f32("SteeringWheelAngle");
        f.yaw_rate = self.f32("YawRate");
        f.lat_accel = self.f32("LatAccel");
        f.long_accel = self.f32("LongAccel");
        f.vert_accel = self.f32("VertAccel");
        f.pitch = self.f32("Pitch");
        f.roll = self.f32("Roll");
        f.velocity_x = self.f32("VelocityX");
        f.velocity_y = self.f32("VelocityY");

        let opt = |name: &str| self.num(name, 0).map(|v| v as f32);
        f.sl_last_rpm = opt("PlayerCarSLLastRPM").filter(|&r| r > 0.0);
        f.sl_blink_rpm = opt("PlayerCarSLBlinkRPM").filter(|&r| r > 0.0);
        f.engine_warnings = self.num("EngineWarnings", 0).unwrap_or(0.0) as u32;
        f.oil_temp = opt("OilTemp");
        f.oil_press = opt("OilPress");
        f.water_temp = opt("WaterTemp");
        f.fuel_press = opt("FuelPress");
        f.voltage = opt("Voltage");
        f.brake_bias = opt("dcBrakeBias");
        f.brake_bias_fine = opt("dcBrakeBiasFine");
        f.peak_brake_bias = opt("dcPeakBrakeBias");

        f.lap = self.i32("Lap");
        f.lap_completed = self.i32("LapCompleted");
        f.lap_dist_pct = self.f32("LapDistPct");
        f.lap_current = self.f32("LapCurrentLapTime");
        f.lap_last = self.f32("LapLastLapTime");
        f.lap_best = self.f32("LapBestLapTime");
        let delta = |name: &str, ok: &str| self.bool(ok).then(|| self.f32(name));
        f.deltas = Deltas {
            best: delta("LapDeltaToBestLap", "LapDeltaToBestLap_OK"),
            optimal: delta("LapDeltaToOptimalLap", "LapDeltaToOptimalLap_OK"),
            session_best: delta("LapDeltaToSessionBestLap", "LapDeltaToSessionBestLap_OK"),
            session_optimal: delta("LapDeltaToSessionOptimalLap", "LapDeltaToSessionOptimalLap_OK"),
            // (sic) iRacing's variable name really has the extra "l".
            last: delta("LapDeltaToSessionLastlLap", "LapDeltaToSessionLastlLap_OK"),
            reference: None,
        };

        f.fuel_level = self.f32("FuelLevel");
        f.fuel_level_pct = self.f32("FuelLevelPct");
        f.car_left_right = self.i32("CarLeftRight");
        f.incidents = self.i32("PlayerCarMyIncidentCount");
        f.team_incidents = self.i32("PlayerCarTeamIncidentCount");
        f.fast_repairs_used = self.i32("FastRepairUsed");
        f.track_temp = self.f32("TrackTempCrew");
        f.air_temp = self.f32("AirTemp");

        f.time_of_day = opt("SessionTimeOfDay");
        f.skies = self.num("Skies", 0).map(|v| v as i32);
        f.track_wetness = self.num("TrackWetness", 0).map(|v| v as i32);
        f.declared_wet = self.bool("WeatherDeclaredWet");
        f.wind_speed = opt("WindVel");
        f.wind_dir = opt("WindDir");
        // Documented as a percentage but published as a fraction; accept either.
        f.humidity = opt("RelativeHumidity").map(|h| if h > 1.0 { h / 100.0 } else { h });
        // Same story for these: accept a fraction or a percentage.
        let frac = |v: f32| if v > 1.0 { v / 100.0 } else { v };
        f.precipitation = opt("Precipitation").map(frac);
        f.fog_level = opt("FogLevel").map(frac);
        f.yaw_north = opt("YawNorth");

        for k in 0..4 {
            for j in 0..3 {
                f.tire_temp[k][j] = self.f32(TIRE_TEMP_VARS[k][j]);
                f.tire_wear[k][j] = self.f32(TIRE_WEAR_VARS[k][j]);
            }
            f.tire_cold_pressure[k] = self.f32(TIRE_PRESSURE_VARS[k]);
        }
        f.tire_compound = self.i32("PlayerTireCompound");
        f.tire_sets_used = self.i32("TireSetsUsed");
        f.tire_sets_available = self.num("TireSetsAvailable", 0).map(|v| v as i32).unwrap_or(255);
        f.in_pit_stall = self.bool("PlayerCarInPitStall");

        // Not published while spectating.
        f.pit_service = self.var("PitSvFlags").map(|_| PitService {
            flags: self.num("PitSvFlags", 0).unwrap_or(0.0) as u32,
            fuel: self.f32("PitSvFuel"),
            pressure: PIT_PRESSURE_VARS.map(|name| self.f32(name)),
            compound: self.i32("PitSvTireCompound"),
        });
        f.pit_sv_status = self.i32("PlayerCarPitSvStatus");
        f.fast_repairs_available = self.num("FastRepairAvailable", 0).map(|v| v as i32);

        let n = MAX_CARS;
        f.car_lap.resize(n, 0);
        f.car_lap_completed.resize(n, 0);
        f.car_lap_dist_pct.resize(n, -1.0);
        f.car_position.resize(n, 0);
        f.car_class_position.resize(n, 0);
        f.car_on_pit_road.resize(n, false);
        f.car_track_surface.resize(n, -1);
        f.car_tire_compound.resize(n, -1);
        f.car_est_time.resize(n, 0.0);
        f.car_f2_time.resize(n, 0.0);
        f.car_last_lap.resize(n, -1.0);
        f.car_best_lap.resize(n, -1.0);

        self.fill("CarIdxLap", &mut f.car_lap, |v| v as i32);
        self.fill("CarIdxLapCompleted", &mut f.car_lap_completed, |v| v as i32);
        self.fill("CarIdxLapDistPct", &mut f.car_lap_dist_pct, |v| v as f32);
        self.fill("CarIdxPosition", &mut f.car_position, |v| v as i32);
        self.fill("CarIdxClassPosition", &mut f.car_class_position, |v| v as i32);
        self.fill("CarIdxOnPitRoad", &mut f.car_on_pit_road, |v| v != 0.0);
        self.fill("CarIdxTrackSurface", &mut f.car_track_surface, |v| v as i32);
        self.fill("CarIdxTireCompound", &mut f.car_tire_compound, |v| v as i32);
        self.fill("CarIdxEstTime", &mut f.car_est_time, |v| v as f32);
        self.fill("CarIdxF2Time", &mut f.car_f2_time, |v| v as f32);
        self.fill("CarIdxLastLapTime", &mut f.car_last_lap, |v| v as f32);
        self.fill("CarIdxBestLapTime", &mut f.car_best_lap, |v| v as f32);
    }
}

impl Drop for Irsdk {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.event);
            UnmapViewOfFile(self.view);
            CloseHandle(self.map);
        }
    }
}
