use winit::window::Window;

/// DWM window attribute for system backdrop type.
/// Available on Windows 11 Build 22000+.
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;

/// DWM window attribute for immersive dark mode.
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

/// System backdrop type values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
// BackdropType::None is defined for completeness of the DWM API enum but unused in code paths.
#[allow(dead_code)]
enum BackdropType {
    /// No system backdrop (Win10 fallback).
    None = 0,
    /// Mica effect (Win11 22000+).
    Mica = 2,
    /// Mica Alt effect (Win11 22H2 / Build 22621+).
    MicaAlt = 4,
}

/// Result of applying window effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectResult {
    /// Mica Alt applied (Win11 22H2+).
    MicaAlt,
    /// Mica applied (Win11).
    Mica,
    /// Opaque fallback (Win10 or unsupported).
    Opaque,
}

/// Apply visual effects (Mica/Acrylic) to the window.
///
/// Detects the Windows version and applies the best available effect:
/// - Win11 22H2+ (Build 22621): Mica Alt
/// - Win11 (Build 22000): Mica
/// - Win10 / older: opaque fallback (no-op)
///
/// Also applies dark mode title bar if `dark_mode` is true.
pub fn apply_window_effects(window: &Window, dark_mode: bool) -> EffectResult {
    #[cfg(target_os = "windows")]
    {
        apply_effects_windows(window, dark_mode)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, dark_mode);
        tracing::debug!("window effects not available on this platform");
        EffectResult::Opaque
    }
}

#[cfg(target_os = "windows")]
fn apply_effects_windows(window: &Window, dark_mode: bool) -> EffectResult {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
    use winit::raw_window_handle::HasWindowHandle;
    use winit::raw_window_handle::RawWindowHandle;

    let hwnd = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut _),
            _ => {
                tracing::warn!("unexpected window handle type, skipping effects");
                return EffectResult::Opaque;
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "failed to get window handle");
            return EffectResult::Opaque;
        }
    };

    // Apply dark mode title bar
    if dark_mode {
        let value: u32 = 1;
        // SAFETY: DwmSetWindowAttribute with DWMWA_USE_IMMERSIVE_DARK_MODE is safe
        // when hwnd is a valid window handle. The value is a u32 boolean (0 or 1).
        let hr = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(DWMWA_USE_IMMERSIVE_DARK_MODE as i32),
                &value as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if let Err(e) = hr {
            tracing::debug!(error = %e, "dark mode DwmSetWindowAttribute failed");
        }
    }

    // Detect Windows version via build number
    let build = windows_build_number();

    let (backdrop, result) = if build >= 22621 {
        (BackdropType::MicaAlt, EffectResult::MicaAlt)
    } else if build >= 22000 {
        (BackdropType::Mica, EffectResult::Mica)
    } else {
        tracing::info!(build, "Win10 detected, using opaque fallback");
        return EffectResult::Opaque;
    };

    let value = backdrop as u32;
    // SAFETY: DwmSetWindowAttribute with DWMWA_SYSTEMBACKDROP_TYPE is safe on
    // Win11 22000+. The hwnd is valid (from winit) and value is a valid enum constant.
    let hr = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(DWMWA_SYSTEMBACKDROP_TYPE as i32),
            &value as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        )
    };

    match hr {
        Ok(()) => {
            tracing::info!(?result, build, "window effects applied");
            result
        }
        Err(e) => {
            tracing::warn!(error = %e, build, "DwmSetWindowAttribute failed, using opaque fallback");
            EffectResult::Opaque
        }
    }
}

// SAFETY: RtlGetVersion is the correct API for version detection on modern
// Windows. GetVersionExW is deprecated and lies on Win8.1+. RtlGetVersion
// always returns accurate info.
#[cfg(target_os = "windows")]
#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(
        lpVersionInformation: *mut windows::Win32::System::SystemInformation::OSVERSIONINFOW,
    ) -> i32;
}

/// Get the Windows build number.
#[cfg(target_os = "windows")]
fn windows_build_number() -> u32 {
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };

    // SAFETY: RtlGetVersion fills the struct when dwOSVersionInfoSize is set correctly.
    let status = unsafe { RtlGetVersion(&mut info) };

    if status == 0 {
        tracing::debug!(
            major = info.dwMajorVersion,
            minor = info.dwMinorVersion,
            build = info.dwBuildNumber,
            "Windows version detected"
        );
        info.dwBuildNumber
    } else {
        tracing::warn!("RtlGetVersion failed, assuming Win10");
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_result_debug() {
        let r = EffectResult::MicaAlt;
        assert_eq!(format!("{r:?}"), "MicaAlt");
    }

    #[test]
    fn backdrop_type_values() {
        assert_eq!(BackdropType::None as u32, 0);
        assert_eq!(BackdropType::Mica as u32, 2);
        assert_eq!(BackdropType::MicaAlt as u32, 4);
    }
}
