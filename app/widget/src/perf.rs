use serde::Serialize;
use windows::Win32::System::ProcessStatus::{
    EmptyWorkingSet, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct MemoryStats {
    pub working_set: u64,
    pub private_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct CpuTimes {
    pub kernel_ms: f64,
    pub user_ms: f64,
}

impl CpuTimes {
    pub fn total_ms(self) -> f64 {
        self.kernel_ms + self.user_ms
    }
}

pub fn memory_stats() -> MemoryStats {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    // SAFETY: the output struct is sized and the process handle is the current pseudo handle.
    unsafe {
        let _ = GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
            counters.cb,
        );
    }
    MemoryStats {
        working_set: counters.WorkingSetSize as u64,
        private_bytes: counters.PrivateUsage as u64,
    }
}

pub fn trim_working_set() {
    // SAFETY: EmptyWorkingSet accepts the pseudo handle for the current process.
    unsafe {
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

pub fn cpu_times() -> CpuTimes {
    let mut creation = Default::default();
    let mut exit = Default::default();
    let mut kernel = Default::default();
    let mut user = Default::default();
    // SAFETY: all FILETIME outputs are initialized by the call.
    unsafe {
        let _ = GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        );
    }
    let to_ms = |time: windows::Win32::Foundation::FILETIME| {
        ((time.dwHighDateTime as u64) << 32 | time.dwLowDateTime as u64) as f64 / 10_000.0
    };
    CpuTimes {
        kernel_ms: to_ms(kernel),
        user_ms: to_ms(user),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_is_reported() {
        let memory = memory_stats();
        assert!(memory.working_set > 0);
        assert!(memory.private_bytes > 0);
    }

    #[test]
    fn cpu_times_are_finite() {
        let cpu = cpu_times();
        assert!(cpu.kernel_ms.is_finite());
        assert!(cpu.user_ms.is_finite());
        assert!(cpu.total_ms() >= 0.0);
    }
}
