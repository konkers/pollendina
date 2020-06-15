use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "macos")]
fn os_specific() {
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.13");
}

#[cfg(target_os = "windows")]
fn os_specific() {
    // Create the wxs file listing everything in the mods directory.
    let mut heat_path = PathBuf::from(
        std::env::var("WIX")
            .expect("Wix not installed.  Please install at https://wixtoolset.org/"),
    );
    heat_path.push("bin");
    heat_path.push("heat.exe");
    Command::new(heat_path)
        .args(&[
            "dir",
            "mods",
            "-dr",
            "MODS",
            "-cg",
            "ModsGroup",
            "-gg",
            "-ke",
            "-sfrag",
            "-srd",
            "-platform",
            "x64",
            "-var",
            "var.ModsSource",
            "-template",
            "fragment",
            "-out",
            "target/wix/mods.wxs",
        ])
        .output()
        .expect("failed to run heat to generate mods list");

    // Add our icon to the exe.
    let mut res = winres::WindowsResource::new();
    res.set_icon("resources/icon.ico");
    res.compile().unwrap();
}

#[cfg(target_os = "linux")]
fn os_specific() {}

fn main() {
    os_specific();
}
