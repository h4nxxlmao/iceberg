use serde::Serialize;
use std::collections::HashSet;

use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{
    EmptyWorkingSet, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, ProcessPowerThrottling, SetProcessInformation,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
};

use crate::state::AppState;

#[derive(Clone, Debug, Default, Serialize)]
pub struct ProcessStats {
    pub working_set: u64,
    pub private_bytes: u64,
    pub process_count: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostics {
    pub ram_bytes: u64,
    pub private_bytes: u64,
    pub process_count: u32,
    pub webview_cache_bytes: u64,
    pub lyrics_cache_bytes: u64,
    pub config_path: String,
    pub webview_path: String,
    pub active_source: String,
    pub active_title: String,
    pub cpu_note: String,
}

pub fn set_efficiency_mode() {
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    };
    // SAFETY: the state is a fully initialized struct and the handle is the current process.
    unsafe {
        let _ = SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            &state as *const PROCESS_POWER_THROTTLING_STATE as *const core::ffi::c_void,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

pub fn trim_working_set() {
    // SAFETY: EmptyWorkingSet accepts the pseudo handle for the current process.
    unsafe {
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

pub fn trim_process_tree() {
    let me = std::process::id();
    let mut parents: Vec<(u32, u32)> = Vec::new();

    // SAFETY: the snapshot handle is checked by the API wrapper and closed before returning.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            trim_working_set();
            return;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                parents.push((entry.th32ProcessID, entry.th32ParentProcessID));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }

    let mut wanted: HashSet<u32> = HashSet::new();
    wanted.insert(me);
    let mut changed = true;
    while changed {
        changed = false;
        for (pid, parent) in &parents {
            if wanted.contains(parent) && wanted.insert(*pid) {
                changed = true;
            }
        }
    }

    for pid in wanted {
        // SAFETY: handles are opened with the access rights needed by EmptyWorkingSet and closed.
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA, false, pid)
            else {
                continue;
            };
            let _ = EmptyWorkingSet(handle);
            let _ = CloseHandle(handle);
        }
    }
}

pub fn process_stats() -> ProcessStats {
    let me = std::process::id();
    let mut parents: Vec<(u32, u32)> = Vec::new();

    // SAFETY: the snapshot handle is checked by the API wrapper and closed before returning.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return ProcessStats::default();
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                parents.push((entry.th32ProcessID, entry.th32ParentProcessID));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }

    let mut wanted: HashSet<u32> = HashSet::new();
    wanted.insert(me);
    let mut changed = true;
    while changed {
        changed = false;
        for (pid, parent) in &parents {
            if wanted.contains(parent) && wanted.insert(*pid) {
                changed = true;
            }
        }
    }

    let mut stats = ProcessStats::default();
    for pid in wanted {
        // SAFETY: the handle is opened for the requested pid and closed after use.
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) else {
                continue;
            };
            let mut counters: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
            if GetProcessMemoryInfo(
                handle,
                &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
                counters.cb,
            )
            .is_ok()
            {
                stats.working_set = stats
                    .working_set
                    .saturating_add(counters.WorkingSetSize as u64);
                stats.private_bytes = stats
                    .private_bytes
                    .saturating_add(counters.PrivateUsage as u64);
                stats.process_count += 1;
            }
            let _ = CloseHandle(handle);
        }
    }

    stats
}

pub fn diagnostics(app: &AppHandle) -> Diagnostics {
    let state = app.state::<AppState>();
    let stats = process_stats();
    let track = state.current_track();
    Diagnostics {
        ram_bytes: stats.working_set,
        private_bytes: stats.private_bytes,
        process_count: stats.process_count,
        webview_cache_bytes: crate::util::dir_size(&state.webview_dir),
        lyrics_cache_bytes: crate::cache::cache_size(&state.cache_dir),
        config_path: state.config_path.display().to_string(),
        webview_path: state.webview_dir.display().to_string(),
        active_source: track
            .as_ref()
            .map(|track| track.source_app.clone())
            .unwrap_or_default(),
        active_title: track
            .as_ref()
            .map(|track| format!("{} by {}", track.title, track.artist))
            .unwrap_or_default(),
        cpu_note: "RAM covers iceberg.exe and every msedgewebview2 child. CPU is measured outside the app.".to_string(),
    }
}
