use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

/// Keeps a named mutex alive. Dropping it releases the single-instance claim.
pub struct InstanceGuard {
    handle: HANDLE,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: the handle was created by acquire() and is closed exactly once.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub fn acquire(name: &str) -> Option<InstanceGuard> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the name is a valid NUL terminated UTF-16 string and the handle is owned here.
    unsafe {
        let handle = CreateMutexW(None, true, PCWSTR(wide.as_ptr())).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(InstanceGuard { handle })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    #[test]
    fn second_acquire_is_rejected_and_release_allows_a_new_one() {
        let name = format!(
            "iceberg-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let first = acquire(&name).expect("first instance");
        assert!(acquire(&name).is_none());
        drop(first);
        assert!(acquire(&name).is_some());
    }
}
