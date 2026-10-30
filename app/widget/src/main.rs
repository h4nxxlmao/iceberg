#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod demo;
mod dpi;
mod perf;
mod render;
mod shot;
mod tray;
mod window;

use std::path::PathBuf;
use std::time::Duration;

use iceberg_core::config::WidgetMode;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("iceberg: {error}");
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(error) = dpi::enable_per_monitor_v2() {
        eprintln!("iceberg: {error}");
    }
    if let Err(error) = app::run(options) {
        eprintln!("iceberg: {error}");
        std::process::exit(1);
    }
}

const USAGE: &str = "\
Usage: iceberg [options]

  --demo                 show the built-in demo track instead of Windows media
  --mode=card|lyrics|island
  --scale=100|150|200    override the monitor scale for layout testing
  --click-through        start with mouse input passing through the widget
  --screenshot=DIR       render review screenshots and exit
  --bench=FILE           run the cached-text transition benchmark and exit
  --quit-after=SECONDS   exit automatically
  --config=FILE          use a specific config file
";

fn parse_args(args: &[String]) -> Result<app::Options, String> {
    let mut options = app::Options::default();
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--demo" => options.demo = true,
            "--click-through" => options.click_through = Some(true),
            "--screenshot" => options.screenshot = Some(PathBuf::from("docs/phase-a")),
            "--bench" => options.bench = Some(PathBuf::from("bench.json")),
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            _ => {
                if let Some(value) = arg.strip_prefix("--mode=") {
                    options.mode = Some(match value {
                        "card" => WidgetMode::Card,
                        "lyrics" => WidgetMode::Lyrics,
                        "island" => WidgetMode::Island,
                        other => return Err(format!("unknown mode '{other}'")),
                    });
                } else if let Some(value) = arg.strip_prefix("--scale=") {
                    let parsed: f64 = value
                        .parse()
                        .map_err(|_| format!("invalid scale '{value}'"))?;
                    let os = if parsed > 4.0 { parsed / 100.0 } else { parsed };
                    if !(1.0..=4.0).contains(&os) {
                        return Err("scale must be between 1.0 and 4.0 (or 100-400%)".to_string());
                    }
                    options.scale_override = Some(os);
                } else if let Some(value) = arg.strip_prefix("--screenshot=") {
                    options.screenshot = Some(PathBuf::from(value));
                } else if let Some(value) = arg.strip_prefix("--bench=") {
                    options.bench = Some(PathBuf::from(value));
                } else if let Some(value) = arg.strip_prefix("--quit-after=") {
                    let seconds: u64 = value
                        .parse()
                        .map_err(|_| format!("invalid quit-after '{value}'"))?;
                    options.quit_after = Some(Duration::from_secs(seconds));
                } else if let Some(value) = arg.strip_prefix("--config=") {
                    options.config_path = Some(PathBuf::from(value));
                } else {
                    return Err(format!("unknown argument '{arg}'"));
                }
            }
        }
    }
    Ok(options)
}
