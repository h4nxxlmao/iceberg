use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

fn main() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is inside app");
    let task = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets".to_string());
    match task.as_str() {
        "assets" => {
            if let Err(error) = render_assets(app) {
                fail(&error);
            }
        }
        "package" => {
            if let Err(error) = package(app) {
                fail(&error);
            }
        }
        other => {
            eprintln!("usage: cargo xtask [assets|package] (got '{other}')");
            std::process::exit(2);
        }
    }
}

fn fail(message: &str) -> ! {
    eprintln!("xtask: {message}");
    std::process::exit(1);
}

fn run(command: &mut Command) -> Result<(), String> {
    let label = format!("{command:?}");
    let status = command
        .status()
        .map_err(|error| format!("{label}: {error}"))?;
    check(status, &label)
}

fn check(status: ExitStatus, label: &str) -> Result<(), String> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} exited with {status}"))
    }
}

fn render_assets(app: &Path) -> Result<(), String> {
    run(Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "-p",
            "iceberg-widget",
            "--",
            "--settings-assets=assets",
        ])
        .current_dir(app))?;
    println!("Wrote app/assets/presets and app/assets/surfaces");
    Ok(())
}

fn package(app: &Path) -> Result<(), String> {
    let settings = app.join("settings");
    let target = app.join("target");
    println!("Building frontend...");
    run(Command::new("npm.cmd")
        .args(["run", "build"])
        .current_dir(&settings))?;
    println!("Building native widget...");
    run(Command::new("cargo")
        .args(["build", "--release", "-p", "iceberg-widget"])
        .current_dir(app))?;
    println!("Building settings app...");
    run(Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "iceberg-settings",
            "--features",
            "custom-protocol",
        ])
        .current_dir(app))?;

    let release = target.join("release");
    let widget = release.join("iceberg.exe");
    let settings_exe = release.join("iceberg-settings.exe");
    if !widget.is_file() || !settings_exe.is_file() {
        return Err("release binaries are missing".to_string());
    }

    let portable = app.join("dist").join("Iceberg");
    if portable.exists() {
        fs::remove_dir_all(&portable).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&portable).map_err(|error| error.to_string())?;
    fs::copy(&widget, portable.join("iceberg.exe")).map_err(|error| error.to_string())?;
    fs::copy(&settings_exe, portable.join("iceberg-settings.exe"))
        .map_err(|error| error.to_string())?;
    copy_dir(&app.join("assets"), &portable.join("assets"))?;
    copy_file(&app.join("..").join("LICENSE"), &portable.join("LICENSE"))?;
    copy_file(
        &app.join("..").join("README.md"),
        &portable.join("README.md"),
    )?;

    let sidecar = settings
        .join("src-tauri")
        .join("binaries")
        .join("iceberg-x86_64-pc-windows-msvc.exe");
    if let Some(parent) = sidecar.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::copy(&widget, &sidecar).map_err(|error| error.to_string())?;
    let resources = settings.join("src-tauri").join("resources");
    if resources.exists() {
        fs::remove_dir_all(&resources).map_err(|error| error.to_string())?;
    }
    copy_dir(&app.join("assets"), &resources.join("assets"))?;

    println!("Portable build: {}", portable.display());
    println!("Building NSIS installer...");
    let outcome = run(Command::new("npm.cmd")
        .args([
            "run",
            "tauri",
            "--",
            "build",
            "--config",
            "src-tauri/tauri.release.conf.json",
        ])
        .current_dir(&settings));
    if let Err(error) = outcome {
        eprintln!("xtask: installer step failed: {error}");
        eprintln!("The portable folder is still available.");
        return Ok(());
    }
    if let Some(installer) = find_installer(&target) {
        println!("Installer: {}", installer.display());
    } else {
        eprintln!("xtask: installer was built but not found under target/release/bundle");
    }
    Ok(())
}

fn find_installer(target: &Path) -> Option<PathBuf> {
    for base in [
        target.join("release").join("bundle").join("nsis"),
        target.join("release").join("bundle"),
    ] {
        let Ok(entries) = fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("exe") {
                return Some(path);
            }
        }
    }
    None
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", from.display()))
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(from).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)
                .map(|_| ())
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}
