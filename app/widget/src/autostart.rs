use windows::core::w;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn apply(enabled: bool) {
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return,
    };
    let key_path: Vec<u16> = RUN_KEY.encode_utf16().chain(Some(0)).collect();
    let mut key = HKEY::default();
    // SAFETY: the Run key path is NUL terminated and the returned key is closed below.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(key_path.as_ptr()),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        return;
    }
    if enabled {
        let command = format!("\"{}\"", executable.display());
        let wide: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
        let bytes: Vec<u8> = wide.iter().flat_map(|unit| unit.to_le_bytes()).collect();
        // SAFETY: key is open for writes; the byte buffer lives through the call.
        let status = unsafe { RegSetValueExW(key, w!("Iceberg"), None, REG_SZ, Some(&bytes)) };
        if status != ERROR_SUCCESS {
            eprintln!("iceberg: could not enable start with Windows ({status:?})");
        }
    } else {
        // SAFETY: key is open for writes; a missing value is fine.
        unsafe {
            let _ = RegDeleteValueW(key, w!("Iceberg"));
        }
    }
    // SAFETY: the key was opened above and is no longer used.
    unsafe {
        let _ = RegCloseKey(key);
    }
}
