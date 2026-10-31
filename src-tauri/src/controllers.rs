//! Buttons on wheels, button boxes and gamepads, read with Windows Raw Input.
//!
//! Raw Input keeps arriving while iRacing has the focus (RIDEV_INPUTSINK) and
//! reports every button a device has, where the old joystick API stops at 32.
//! A hidden message-only window on its own thread receives WM_INPUT; each
//! report is decoded with the device's HID descriptor (HidP_GetUsages) into
//! the buttons held down, and the newly pressed ones are passed on. A hat
//! switch (the d-pad on most wheels) becomes four more buttons: 1001 up,
//! 1002 right, 1003 down, 1004 left.
//!
//! Devices are named by the hardware id in their path ("VID_0EB7&PID_0E04&MI_00"),
//! which stays the same across USB ports and restarts.

use serde::Serialize;

use crate::controls::ButtonPress;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Controller {
    pub device: String,
    pub name: String,
}

#[cfg(not(windows))]
pub fn spawn(_on_press: impl Fn(ButtonPress) + Send + 'static) {}

#[cfg(not(windows))]
pub fn list() -> Vec<Controller> {
    Vec::new()
}

#[cfg(windows)]
pub use win::{list, spawn};

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::collections::hash_map::Entry;
    use std::collections::HashMap;
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Devices::HumanInterfaceDevice::{
        HidD_GetProductString, HidP_GetCaps, HidP_GetUsageValue, HidP_GetUsages, HidP_GetValueCaps, HidP_Input,
        HidP_MaxUsageListLength, HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::{
        GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList, RegisterRawInputDevices, RAWINPUT,
        RAWINPUTDEVICE, RAWINPUTDEVICELIST, RAWINPUTHEADER, RID_DEVICE_INFO, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK,
        RIDI_DEVICEINFO, RIDI_DEVICENAME, RIDI_PREPARSEDDATA, RID_INPUT, RIM_TYPEHID,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, HWND_MESSAGE, MSG, WM_INPUT,
        WM_INPUT_DEVICE_CHANGE, WNDCLASSW,
    };

    use super::{ButtonPress, Controller};

    /// HID usage page 1 (generic desktop): joystick, gamepad, multi-axis controller.
    const USAGE_PAGE_GENERIC: u16 = 0x01;
    const CONTROLLER_USAGES: [u16; 3] = [0x04, 0x05, 0x08];
    const USAGE_PAGE_BUTTON: u16 = 0x09;
    const USAGE_HAT: u16 = 0x39;
    /// WM_INPUT_DEVICE_CHANGE's wParam when a device goes away.
    const GIDC_REMOVAL: usize = 2;
    pub const HAT_UP: u32 = 1001;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Device {
        id: String,
        name: String,
        preparsed: Vec<u8>,
        max_usages: usize,
        /// Logical range of the hat switch, if the device has one.
        hat: Option<(i32, i32)>,
        /// Buttons held at the last report.
        held: Vec<u32>,
    }

    struct State {
        devices: HashMap<isize, Device>,
        on_press: Box<dyn Fn(ButtonPress)>,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    /// Starts the thread that reads controller buttons and calls `on_press` for each press.
    pub fn spawn(on_press: impl Fn(ButtonPress) + Send + 'static) {
        let spawned = std::thread::Builder::new().name("controllers".into()).spawn(move || unsafe {
            STATE.with(|s| *s.borrow_mut() = Some(State { devices: HashMap::new(), on_press: Box::new(on_press) }));
            let Some(hwnd) = message_window() else {
                eprintln!("[tenths] controllers: no window for Raw Input");
                return;
            };
            let devices = CONTROLLER_USAGES.map(|usage| RAWINPUTDEVICE {
                usUsagePage: USAGE_PAGE_GENERIC,
                usUsage: usage,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: hwnd,
            });
            if RegisterRawInputDevices(devices.as_ptr(), devices.len() as u32, size_of::<RAWINPUTDEVICE>() as u32) == 0 {
                eprintln!("[tenths] controllers: RegisterRawInputDevices failed");
                return;
            }
            let mut msg: MSG = zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        });
        if let Err(err) = spawned {
            eprintln!("[tenths] controllers: {err}");
        }
    }

    unsafe fn message_window() -> Option<HWND> {
        let class = wide("TenthsControllers");
        let instance = GetModuleHandleW(null());
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, null_mut(), instance, null());
        (!hwnd.is_null()).then_some(hwnd)
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_INPUT => {
                on_input(lparam);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_INPUT_DEVICE_CHANGE => {
                // Handles can be reused by the next device plugged in.
                if wparam == GIDC_REMOVAL {
                    STATE.with(|s| {
                        if let Some(state) = s.borrow_mut().as_mut() {
                            state.devices.remove(&lparam);
                        }
                    });
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn on_input(lparam: LPARAM) {
        let header = size_of::<RAWINPUTHEADER>() as u32;
        let mut size = 0u32;
        GetRawInputData(lparam as _, RID_INPUT, null_mut(), &mut size, header);
        if size == 0 {
            return;
        }
        // u64s keep RAWINPUT aligned.
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        if GetRawInputData(lparam as _, RID_INPUT, buf.as_mut_ptr().cast(), &mut size, header) != size {
            return;
        }
        let raw = &*(buf.as_ptr() as *const RAWINPUT);
        if raw.header.dwType != RIM_TYPEHID {
            return;
        }
        let hid = &raw.data.hid;
        let start = std::ptr::addr_of!(hid.bRawData) as usize - buf.as_ptr() as usize;
        let bytes = std::slice::from_raw_parts(buf.as_ptr() as *const u8, size as usize);
        let report_len = hid.dwSizeHid as usize;
        let handle = raw.header.hDevice;

        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let Some(state) = s.as_mut() else { return };
            let dev = match state.devices.entry(handle as isize) {
                Entry::Occupied(e) => e.into_mut(),
                Entry::Vacant(e) => match device(handle) {
                    Some(d) => e.insert(d),
                    None => return,
                },
            };
            for i in 0..hid.dwCount as usize {
                let Some(report) = bytes.get(start + i * report_len..start + (i + 1) * report_len) else { break };
                let held = held(dev, report);
                for &b in held.iter().filter(|b| !dev.held.contains(b)) {
                    (state.on_press)(ButtonPress { device: dev.id.clone(), name: dev.name.clone(), button: b });
                }
                dev.held = held;
            }
        });
    }

    /// Buttons held in one report, with the hat as up to two of 1001..1004.
    unsafe fn held(dev: &Device, report: &[u8]) -> Vec<u32> {
        let preparsed = dev.preparsed.as_ptr() as _;
        let mut usages = vec![0u16; dev.max_usages.max(1)];
        let mut n = usages.len() as u32;
        let mut held: Vec<u32> = Vec::new();
        if HidP_GetUsages(HidP_Input, USAGE_PAGE_BUTTON, 0, usages.as_mut_ptr(), &mut n, preparsed, report.as_ptr() as _, report.len() as u32)
            == HIDP_STATUS_SUCCESS
        {
            held.extend(usages[..n as usize].iter().map(|&u| u as u32));
        }
        if let Some((min, max)) = dev.hat {
            let mut value = 0u32;
            if HidP_GetUsageValue(HidP_Input, USAGE_PAGE_GENERIC, 0, USAGE_HAT, &mut value, preparsed, report.as_ptr() as _, report.len() as u32)
                == HIDP_STATUS_SUCCESS
            {
                held.extend(hat_buttons(value as i32, min, max));
            }
        }
        held
    }

    /// A hat position as the directions it points: 8-way hats give two at the diagonals.
    pub fn hat_buttons(value: i32, min: i32, max: i32) -> Vec<u32> {
        if value < min || value > max {
            return Vec::new(); // centred
        }
        let pos = (value - min) as u32;
        let dirs: &[u32] = match (max - min + 1, pos) {
            (8, 0) => &[0],
            (8, 1) => &[0, 1],
            (8, 2) => &[1],
            (8, 3) => &[1, 2],
            (8, 4) => &[2],
            (8, 5) => &[2, 3],
            (8, 6) => &[3],
            (8, 7) => &[3, 0],
            (4, p) if p < 4 => std::slice::from_ref(&[0, 1, 2, 3][p as usize]),
            _ => &[],
        };
        dirs.iter().map(|d| HAT_UP + d).collect()
    }

    unsafe fn device(handle: HANDLE) -> Option<Device> {
        let path = device_path(handle)?;
        let mut size = 0u32;
        GetRawInputDeviceInfoW(handle, RIDI_PREPARSEDDATA, null_mut(), &mut size);
        if size == 0 {
            return None;
        }
        let mut preparsed = vec![0u8; size as usize];
        if GetRawInputDeviceInfoW(handle, RIDI_PREPARSEDDATA, preparsed.as_mut_ptr().cast(), &mut size) == u32::MAX {
            return None;
        }
        let pp = preparsed.as_ptr() as _;
        let max_usages = HidP_MaxUsageListLength(HidP_Input, USAGE_PAGE_BUTTON, pp) as usize;

        let mut caps: HIDP_CAPS = zeroed();
        let mut hat = None;
        if HidP_GetCaps(pp, &mut caps) == HIDP_STATUS_SUCCESS && caps.NumberInputValueCaps > 0 {
            let mut len = caps.NumberInputValueCaps;
            let mut values: Vec<HIDP_VALUE_CAPS> = vec![zeroed(); len as usize];
            if HidP_GetValueCaps(HidP_Input, values.as_mut_ptr(), &mut len, pp) == HIDP_STATUS_SUCCESS {
                hat = values[..len as usize]
                    .iter()
                    .find(|v| {
                        v.UsagePage == USAGE_PAGE_GENERIC
                            && if v.IsRange != 0 {
                                (v.Anonymous.Range.UsageMin..=v.Anonymous.Range.UsageMax).contains(&USAGE_HAT)
                            } else {
                                v.Anonymous.NotRange.Usage == USAGE_HAT
                            }
                    })
                    .map(|v| (v.LogicalMin, v.LogicalMax));
            }
        }
        let id = hardware_id(&path);
        Some(Device { name: product_name(&path).unwrap_or_else(|| format!("Controller {id}")), id, preparsed, max_usages, hat, held: Vec::new() })
    }

    unsafe fn device_path(handle: HANDLE) -> Option<String> {
        let mut chars = 0u32;
        GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, null_mut(), &mut chars);
        if chars == 0 {
            return None;
        }
        let mut name = vec![0u16; chars as usize];
        if GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, name.as_mut_ptr().cast(), &mut chars) == u32::MAX {
            return None;
        }
        let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        Some(String::from_utf16_lossy(&name[..end]))
    }

    /// `\\?\HID#VID_0EB7&PID_0E04&MI_00#7&2b1c...#{guid}` -> `VID_0EB7&PID_0E04&MI_00`.
    pub fn hardware_id(path: &str) -> String {
        path.split('#').nth(1).unwrap_or(path).to_ascii_uppercase()
    }

    unsafe fn product_name(path: &str) -> Option<String> {
        let file = CreateFileW(wide(path).as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, null(), OPEN_EXISTING, 0, null_mut());
        if file == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut buf = [0u16; 127];
        let ok = HidD_GetProductString(file, buf.as_mut_ptr().cast(), (buf.len() * 2) as u32) != 0;
        CloseHandle(file);
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let name = String::from_utf16_lossy(&buf[..end]).trim().to_string();
        (ok && !name.is_empty()).then_some(name)
    }

    /// Wheels, button boxes and gamepads plugged in now.
    pub fn list() -> Vec<Controller> {
        unsafe {
            let mut count = 0u32;
            let entry = size_of::<RAWINPUTDEVICELIST>() as u32;
            GetRawInputDeviceList(null_mut(), &mut count, entry);
            let mut list: Vec<RAWINPUTDEVICELIST> = vec![zeroed(); count as usize];
            let got = GetRawInputDeviceList(list.as_mut_ptr(), &mut count, entry);
            if got == u32::MAX {
                return Vec::new();
            }
            let mut out: Vec<Controller> = Vec::new();
            for d in &list[..got as usize] {
                if d.dwType != RIM_TYPEHID {
                    continue;
                }
                let mut info: RID_DEVICE_INFO = zeroed();
                info.cbSize = size_of::<RID_DEVICE_INFO>() as u32;
                let mut size = info.cbSize;
                if GetRawInputDeviceInfoW(d.hDevice, RIDI_DEVICEINFO, (&mut info as *mut RID_DEVICE_INFO).cast(), &mut size) == u32::MAX {
                    continue;
                }
                let hid = info.Anonymous.hid;
                if hid.usUsagePage != USAGE_PAGE_GENERIC || !CONTROLLER_USAGES.contains(&hid.usUsage) {
                    continue;
                }
                let Some(path) = device_path(d.hDevice) else { continue };
                let device = hardware_id(&path);
                if out.iter().any(|c| c.device == device) {
                    continue;
                }
                let name = product_name(&path).unwrap_or_else(|| format!("Controller {device}"));
                out.push(Controller { device, name });
            }
            out
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn hats_point_where_they_should() {
            assert_eq!(hat_buttons(0, 0, 7), vec![1001]);
            assert_eq!(hat_buttons(3, 0, 7), vec![1002, 1003]);
            assert_eq!(hat_buttons(7, 0, 7), vec![1004, 1001]);
            assert_eq!(hat_buttons(8, 0, 7), Vec::<u32>::new(), "centred");
            assert_eq!(hat_buttons(4, 1, 8), vec![1002, 1003]);
            assert_eq!(hat_buttons(2, 0, 3), vec![1003]);
        }

        #[test]
        fn hardware_id_from_path() {
            assert_eq!(
                hardware_id(r"\\?\HID#VID_0eb7&PID_0e04&MI_00#7&2b1c3e&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"),
                "VID_0EB7&PID_0E04&MI_00"
            );
        }

        /// Prints what's plugged in: `cargo test -p tenths list_controllers -- --nocapture --ignored`.
        #[test]
        #[ignore]
        fn list_controllers() {
            for c in list() {
                println!("{}  {}", c.device, c.name);
            }
        }
    }
}
