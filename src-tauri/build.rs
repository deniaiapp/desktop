use std::{path::Path, process::Command};

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn emit_build_info() {
    let git_sha = command_output("git", &["rev-parse", "--short=8", "HEAD"]);
    let git_date = command_output("git", &["log", "-1", "--format=%cs"]);
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let rustc_version = command_output(&rustc, &["--version"]);
    let target = std::env::var("TARGET").ok();

    for (key, value) in [
        ("DENI_GIT_SHA", git_sha),
        ("DENI_GIT_DATE", git_date),
        ("DENI_RUSTC_VERSION", rustc_version),
        ("DENI_BUILD_TARGET", target),
    ] {
        println!(
            "cargo:rustc-env={key}={}",
            value.unwrap_or_else(|| "unknown".to_string())
        );
    }

    // Re-run when the checked-out commit changes, without forcing a rerun on
    // every build when the .git directory is missing (e.g. source tarballs).
    for path in ["../.git/HEAD", "../.git/refs/heads", "../.git/packed-refs"] {
        if Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

fn main() {
    emit_build_info();
    tauri_build::build()
}
