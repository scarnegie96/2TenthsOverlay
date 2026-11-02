//! Puts the app icon and version information into `tenths.exe` on Windows
//! (what Explorer, the taskbar and the installer's shortcuts show). The version
//! comes from `Cargo.toml`, so a release bump is the only place to change it.
//! Nothing to do on other platforms.

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    // `cfg(windows)` here is the machine building, which is where the resource compiler is.
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icons/icon.ico");
            res.set("ProductName", "Tenths");
            res.set("FileDescription", "Tenths iRacing overlay");
            res.set("OriginalFilename", "tenths.exe");
            res.set("LegalCopyright", "Copyright (C) 2026 Scott Carnegie");
            if let Err(e) = res.compile() {
                // A build without the icon is still a working build.
                println!("cargo:warning=couldn't embed the icon and version info: {e}");
            }
        }
    }
}
