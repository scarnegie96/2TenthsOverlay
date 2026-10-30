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

use std::collections::HashMap;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS,
};
use windows_sys::Win32::System::Threading::{OpenEventW, WaitForSingleObject, SYNCHRONIZATION_SYNCHRONIZE};

use crate::model::{Deltas, RawFrame, MAX_CARS};
use crate::vars::{self, VarHeader, VAR_HEADER_SIZE};

const MEM_MAP_NAME: &str = "Local\\IRSDKMemMapFileName";
const DATA_VALID_EVENT_NAME: &str = "Local\\IRSDKDataValidEvent";
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

/// Connection to the shared memory. Dropping it unmaps and closes the handles.
pub struct Irsdk {
    map: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    event: HANDLE,
    vars: HashMap<String, VarHeader>,
    num_vars: i32,
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
            let event = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, wide(DATA_VALID_EVENT_NAME).as_ptr());
            if event.is_null() {
                UnmapViewOfFile(view);
                CloseHandle(map);
                return None;
            }
            Some(Self {
                map,
                view,
                event,
                vars: HashMap::new(),
                num_vars: -1,
                last_tick: -1,
                last_session_update: -1,
                buf: Vec::new(),
            })
        }
    }

    fn base(&self) -> *const u8 {
        self.view.Value as *const u8
    }

    fn header_i32(&self, offset: usize) -> i32 {
        unsafe { ptr::read_volatile(self.base().add(offset) as *const i32) }
    }

    pub fn is_connected(&self) -> bool {
        self.header_i32(H_STATUS) & STATUS_CONNECTED != 0
    }

    /// Blocks up to `timeout_ms` for a new frame. Returns true when a fresh
    /// buffer has been copied and can be read with the accessors below.
    pub fn wait_for_data(&mut self, timeout_ms: u32) -> bool {
        let signalled = unsafe { WaitForSingleObject(self.event, timeout_ms) } == WAIT_OBJECT_0;
        if !self.is_connected() {
            self.last_tick = -1;
            return false;
        }
        if self.header_i32(H_NUM_VARS) != self.num_vars {
            self.load_var_headers();
        }
        // Even without the signal a new buffer may have landed; checking is cheap.
        let _ = signalled;
        self.copy_latest_buffer()
    }

    fn load_var_headers(&mut self) {
        let num = self.header_i32(H_NUM_VARS).max(0) as usize;
        let base_off = self.header_i32(H_VAR_HEADER_OFFSET).max(0) as usize;
        let mut raw = vec![0u8; num * VAR_HEADER_SIZE];
        unsafe { ptr::copy_nonoverlapping(self.base().add(base_off), raw.as_mut_ptr(), raw.len()) };
        self.vars = vars::parse_headers(&raw, num);
        self.num_vars = num as i32;
    }

    fn copy_latest_buffer(&mut self) -> bool {
        let num_buf = (self.header_i32(H_NUM_BUF).max(1) as usize).min(MAX_BUFS);
        let buf_len = self.header_i32(H_BUF_LEN).max(0) as usize;

        for _attempt in 0..3 {
            let (mut best_tick, mut best_off) = (i32::MIN, 0usize);
            for i in 0..num_buf {
                let tick = self.header_i32(H_VAR_BUF + i * 16);
                if tick > best_tick {
                    best_tick = tick;
                    best_off = self.header_i32(H_VAR_BUF + i * 16 + 4) as usize;
                }
            }
            if best_tick == self.last_tick {
                return false;
            }
            self.buf.resize(buf_len, 0);
            unsafe { ptr::copy_nonoverlapping(self.base().add(best_off), self.buf.as_mut_ptr(), buf_len) };

            // Verify iRacing didn't recycle the buffer mid-copy.
            let still = (0..num_buf).any(|i| {
                self.header_i32(H_VAR_BUF + i * 16) == best_tick
                    && self.header_i32(H_VAR_BUF + i * 16 + 4) as usize == best_off
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
        let len = self.header_i32(H_SESSION_INFO_LEN).max(0) as usize;
        let off = self.header_i32(H_SESSION_INFO_OFFSET).max(0) as usize;
        let mut bytes = vec![0u8; len];
        unsafe { ptr::copy_nonoverlapping(self.base().add(off), bytes.as_mut_ptr(), len) };
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
        f.track_temp = self.f32("TrackTempCrew");
        f.air_temp = self.f32("AirTemp");

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

        let n = MAX_CARS;
        f.car_lap.resize(n, 0);
        f.car_lap_completed.resize(n, 0);
        f.car_lap_dist_pct.resize(n, -1.0);
        f.car_position.resize(n, 0);
        f.car_class_position.resize(n, 0);
        f.car_on_pit_road.resize(n, false);
        f.car_track_surface.resize(n, -1);
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
