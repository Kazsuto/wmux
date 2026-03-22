use winit::window::Window;

/// DWM window attribute for immersive dark mode.
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

/// DWM window attribute for window border color (COLORREF).
const DWMWA_BORDER_COLOR: u32 = 34;

/// DWM window attribute for title bar / caption color (COLORREF).
const DWMWA_CAPTION_COLOR: u32 = 35;

/// DWM window attribute for title bar text color (COLORREF).
const DWMWA_TEXT_COLOR: u32 = 36;

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

/// Title bar colors matching the terminal theme.
pub struct TitleBarColors {
    /// Background color as (R, G, B) in sRGB.
    pub background: (u8, u8, u8),
    /// Text color as (R, G, B) in sRGB.
    pub text: (u8, u8, u8),
    /// Border color as (R, G, B) in sRGB.
    pub border: (u8, u8, u8),
}

/// Apply visual effects to the window.
///
/// Sets dark mode, title bar / border colors from the theme, and detects
/// Mica capability (though Mica is not used for the client area).
pub fn apply_window_effects(
    window: &Window,
    dark_mode: bool,
    colors: &TitleBarColors,
) -> EffectResult {
    #[cfg(target_os = "windows")]
    {
        apply_effects_windows(window, dark_mode, colors)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, dark_mode, colors);
        tracing::debug!("window effects not available on this platform");
        EffectResult::Opaque
    }
}

#[cfg(target_os = "windows")]
fn apply_effects_windows(
    window: &Window,
    dark_mode: bool,
    colors: &TitleBarColors,
) -> EffectResult {
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

    // Helper: set a DWM u32 attribute.
    let set_attr = |attr: u32, value: u32| {
        // SAFETY: DwmSetWindowAttribute is safe when hwnd is a valid window
        // handle (from winit) and the attribute+value combination is valid.
        let hr = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(attr as i32),
                &value as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if let Err(e) = hr {
            tracing::debug!(attr, error = %e, "DwmSetWindowAttribute failed");
        }
    };

    // Helper: convert (R, G, B) to COLORREF (0x00BBGGRR).
    let colorref = |r: u8, g: u8, b: u8| -> u32 { (b as u32) << 16 | (g as u32) << 8 | r as u32 };

    // 1. Dark mode title bar
    if dark_mode {
        set_attr(DWMWA_USE_IMMERSIVE_DARK_MODE, 1);
    }

    // 2. Title bar caption color — matches theme background
    set_attr(
        DWMWA_CAPTION_COLOR,
        colorref(
            colors.background.0,
            colors.background.1,
            colors.background.2,
        ),
    );

    // 3. Title bar text color — matches theme foreground
    set_attr(
        DWMWA_TEXT_COLOR,
        colorref(colors.text.0, colors.text.1, colors.text.2),
    );

    // 4. Window border color — matches theme background for seamless look
    set_attr(
        DWMWA_BORDER_COLOR,
        colorref(colors.border.0, colors.border.1, colors.border.2),
    );

    // Detect Windows version for EffectResult reporting.
    let build = windows_build_number();
    let result = if build >= 22621 {
        EffectResult::MicaAlt
    } else if build >= 22000 {
        EffectResult::Mica
    } else {
        EffectResult::Opaque
    };

    tracing::info!(
        ?result,
        build,
        caption = ?colors.background,
        "window title bar colors applied"
    );
    result
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

/// Check whether Windows animation effects are enabled.
///
/// Returns `false` when the user has disabled animation effects in
/// Windows accessibility settings (`SPI_GETCLIENTAREAANIMATION`).
/// Returns `true` (animations enabled) if the API call fails.
pub fn is_animations_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        is_animations_enabled_windows()
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

#[cfg(target_os = "windows")]
fn is_animations_enabled_windows() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    let mut enabled: i32 = 1;
    // SAFETY: SystemParametersInfoW reads client area animation preference into
    // the provided i32 pointer. The buffer size matches the i32 size.
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut i32 as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    match result {
        Ok(()) => enabled != 0,
        Err(e) => {
            tracing::debug!(error = %e, "SPI_GETCLIENTAREAANIMATION failed, assuming animations enabled");
            true
        }
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
    fn title_bar_colors_struct() {
        let c = TitleBarColors {
            background: (0x0d, 0x11, 0x17),
            text: (0xe6, 0xed, 0xf3),
            border: (0x0d, 0x11, 0x17),
        };
        assert_eq!(c.background, (0x0d, 0x11, 0x17));
    }
}
