/// Read the `AppsUseLightTheme` DWORD from the Windows registry.
///
/// Returns `Some(true)` for light mode, `Some(false)` for dark mode,
/// `None` if the registry value cannot be read.
pub(super) fn read_apps_use_light_theme() -> Option<bool> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
        REG_VALUE_TYPE,
    };

    // Registry path: HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize
    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();

    let mut hkey = windows::Win32::System::Registry::HKEY::default();

    // SAFETY: `subkey` is a valid null-terminated UTF-16 string. `hkey` is a local
    // variable used only within this function and is closed before return.
    let open_result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
    };

    if open_result.is_err() {
        tracing::debug!("AppsUseLightTheme registry key not found, defaulting to dark mode");
        return None;
    }

    let mut data: u32 = 0;
    let mut data_size: u32 = std::mem::size_of::<u32>() as u32;
    let mut reg_type = REG_VALUE_TYPE::default();

    // SAFETY: `data` is a stack-allocated u32 cast to `*mut u8` as required by the
    // Windows API. `data_size` correctly reflects its byte length (4 bytes). `hkey`
    // was successfully opened above and remains valid for this call.
    let query_result = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        )
    };

    // SAFETY: `hkey` was successfully opened above and must be closed exactly once.
    // The return value is intentionally ignored — if close fails there is nothing
    // actionable to do and we still proceed with the queried data.
    unsafe {
        let _ = RegCloseKey(hkey);
    };

    if query_result.is_err() {
        tracing::debug!("failed to query AppsUseLightTheme, defaulting to dark mode");
        return None;
    }

    if reg_type != REG_DWORD {
        tracing::debug!(
            reg_type = reg_type.0,
            "AppsUseLightTheme unexpected type, defaulting to dark mode"
        );
        return None;
    }

    // 0 = dark mode, 1 = light mode
    Some(data != 0)
}
