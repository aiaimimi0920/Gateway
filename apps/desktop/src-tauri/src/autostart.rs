//! A dedicated HKCU Run value, never a shell command or a machine-wide startup entry.
#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    type Key = *mut c_void;
    const HKCU: Key = (0x80000001u32 as i32 as isize) as Key;
    const SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const NAME: &str = "NeuroGatewayDesktop";
    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            key: Key,
            sub: *const u16,
            name: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            bytes: *mut u32,
        ) -> i32;
        fn RegCreateKeyExW(
            key: Key,
            sub: *const u16,
            reserved: u32,
            class: *mut u16,
            options: u32,
            access: u32,
            security: *const c_void,
            result: *mut Key,
            disposition: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: Key,
            name: *const u16,
            reserved: u32,
            kind: u32,
            data: *const u8,
            bytes: u32,
        ) -> i32;
        fn RegDeleteValueW(key: Key, name: *const u16) -> i32;
        fn RegCloseKey(key: Key) -> i32;
    }
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    fn command() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        Ok(format!("\"{}\" --gateway-autostart", exe.display()))
    }
    fn read() -> Result<Option<String>, String> {
        let mut bytes = 0u32;
        let sub = wide(SUBKEY);
        let name = wide(NAME);
        // RRF_RT_REG_SZ prevents expansion or interpretation of arbitrary registry types.
        let status = unsafe {
            RegGetValueW(
                HKCU,
                sub.as_ptr(),
                name.as_ptr(),
                2,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        if status == 2 {
            return Ok(None);
        }
        if status != 0 || bytes > 32768 {
            return Err("Cannot read Gateway autostart entry".into());
        }
        let mut data = vec![0u16; (bytes as usize).div_ceil(2)];
        let status = unsafe {
            RegGetValueW(
                HKCU,
                sub.as_ptr(),
                name.as_ptr(),
                2,
                std::ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != 0 {
            return Err("Cannot read Gateway autostart entry".into());
        }
        Ok(Some(
            String::from_utf16_lossy(&data)
                .trim_end_matches('\0')
                .to_owned(),
        ))
    }
    pub fn enabled() -> Result<bool, String> {
        match read()? {
            None => Ok(false),
            Some(value) if value.starts_with('"') && value.ends_with("\" --gateway-autostart") => {
                Ok(true)
            }
            Some(_) => Err("Gateway autostart belongs to a different installation".into()),
        }
    }
    pub fn set(enabled: bool) -> Result<(), String> {
        let previous = self::enabled()?;
        if !enabled && !previous {
            return Ok(());
        }
        // A versioned/portable package may have moved. Refresh only our tagged entry.
        if enabled && read()?.as_ref() == Some(&command()?) {
            return Ok(());
        }
        let mut key = std::ptr::null_mut();
        let sub = wide(SUBKEY);
        let name = wide(NAME);
        let value = wide(&command()?);
        let status = unsafe {
            RegCreateKeyExW(
                HKCU,
                sub.as_ptr(),
                0,
                std::ptr::null_mut(),
                0,
                2,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err("Cannot open Gateway autostart entry".into());
        }
        // The handle is closed on every branch. Only this installation's dedicated value is changed.
        let status = unsafe {
            let result = if enabled {
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    1,
                    value.as_ptr().cast(),
                    (value.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(key, name.as_ptr())
            };
            RegCloseKey(key);
            result
        };
        if status == 0 {
            Ok(())
        } else {
            Err("Cannot update Gateway autostart entry".into())
        }
    }
}
#[cfg(windows)]
pub use windows::{enabled, set};
#[cfg(not(windows))]
pub fn enabled() -> Result<bool, String> {
    Ok(false)
}
#[cfg(not(windows))]
pub fn set(enabled: bool) -> Result<(), String> {
    if enabled {
        Err("Autostart is supported on Windows only".into())
    } else {
        Ok(())
    }
}
