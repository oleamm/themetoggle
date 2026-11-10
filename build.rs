//! Embeds the application icon and manifest into the Windows executable and
//! bakes a build identity into the binary so a shipped .exe can be traced back
//! to an exact commit.

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    for path in [
        "src",
        "Cargo.toml",
        "app.manifest",
        "assets/themetoggle.ico",
        "assets/tray-light.png",
        "assets/tray-dark.png",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    emit_git_rerun_paths();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/themetoggle.ico")
            .set_manifest_file("app.manifest")
            .set("ProductName", "ThemeToggle")
            .set(
                "FileDescription",
                "ThemeToggle - one-click Windows light/dark theme switcher",
            )
            .set("LegalCopyright", "MIT licensed");
        resource
            .compile()
            .expect("failed to embed ThemeToggle Windows resources");
    }

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".into());
    let revision = git_output(&["rev-parse", "--short=10", "HEAD"]).unwrap_or_else(|| "local".into());
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let build_id = format!("{version}+{revision}.{timestamp}");

    println!("cargo:rustc-env=THEMETOGGLE_BUILD_ID={build_id}");
    if let Ok(out_dir) = std::env::var("OUT_DIR") {
        let _ = fs::write(
            std::path::Path::new(&out_dir).join("build-id.txt"),
            &build_id,
        );
    }
}

fn emit_git_rerun_paths() {
    // `.git` is a directory in a normal checkout but a pointer file in a linked
    // worktree, so let Git resolve both layouts.
    for name in ["HEAD", "packed-refs"] {
        if let Some(path) = git_output(&["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git_output(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git_output(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

fn git_output(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}
