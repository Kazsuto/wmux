use super::types::{ColorPalette, ShadowDepth, UiChrome};

/// Derive UI chrome colors from a terminal color palette.
///
/// Surface elevation uses an adaptive lightness step scaled to the base darkness:
/// `(base_lightness * 0.45).clamp(0.030, 0.055)` — very dark bases (e.g. #131313)
/// get tighter ~3.3% steps for subtle tonal layering, while standard dark themes
/// (e.g. #1e1e1e) keep ~5% steps. This preserves existing themes unchanged.
/// Accent is derived from ANSI blue (palette index 4) with saturation boosted to at
/// least 80%. Text base is boosted 70% toward white (dark themes) or black (light
/// themes) for AAA contrast on chrome surfaces — terminal foreground stays unchanged.
/// Text hierarchy uses 88%/75%/40% alpha steps.
pub fn derive_ui_chrome(palette: &ColorPalette) -> UiChrome {
    let (br, bg, bb) = palette.background;
    let bg_r = br as f32 / 255.0;
    let bg_g = bg as f32 / 255.0;
    let bg_b = bb as f32 / 255.0;

    let (h, s, l) = rgb_to_hsl(bg_r, bg_g, bg_b);

    // Adaptive step: very dark bases get tighter elevation for subtle tonal layering.
    // At L=0.075 (#131313): step ≈ 3.4% → surfaces match "Digital Obsidian" spec.
    // At L=0.118 (#1e1e1e): step ≈ 5.3% → preserves existing theme appearance.
    let elevation_step = (l * 0.45).clamp(0.030, 0.055);

    let surface_at_level = |level: u32| -> [f32; 4] {
        // Dark themes: increase lightness for elevation; light themes: decrease.
        let delta = if l > 0.5 {
            -(level as f32 * elevation_step)
        } else {
            level as f32 * elevation_step
        };
        let new_l = (l + delta).clamp(0.05, 0.95);
        let (r, g, b) = hsl_to_rgb(h, s, new_l);
        [r, g, b, 1.0]
    };

    let surface_base = [bg_r, bg_g, bg_b, 1.0];
    let surface_0 = surface_at_level(1); // +1 step
    let surface_1 = surface_at_level(2); // +2 steps
    let surface_2 = surface_at_level(3); // +3 steps
    let surface_3 = surface_at_level(4); // +4 steps
    let surface_overlay = [surface_1[0], surface_1[1], surface_1[2], 0.95];

    // Accent from ANSI blue (index 4), boost saturation to at least 80%
    let (ar, ag, ab) = palette.ansi[4];
    let (ah, a_s, al) = rgb_to_hsl(ar as f32 / 255.0, ag as f32 / 255.0, ab as f32 / 255.0);
    let boosted_s = a_s.max(0.8);
    let (r, g, b) = hsl_to_rgb(ah, boosted_s, al);
    let accent = [r, g, b, 1.0];

    // Accent hover: shift lightness toward visible contrast (like surface elevation)
    let hover_delta = if al > 0.5 { -0.08 } else { 0.08 };
    let hover_l = (al + hover_delta).clamp(0.05, 0.95);
    let (rh, gh, bh) = hsl_to_rgb(ah, boosted_s, hover_l);
    let accent_hover = [rh, gh, bh, 1.0];

    // Accent pressed: shift lightness toward background (opposite of hover)
    let pressed_delta = if al > 0.5 { 0.10 } else { -0.10 };
    let pressed_l = (al + pressed_delta).clamp(0.05, 0.95);
    let (rp, gp, bp) = hsl_to_rgb(ah, boosted_s, pressed_l);
    let accent_pressed = [rp, gp, bp, 1.0];

    let accent_muted = [r, g, b, 0.30];
    let accent_glow = [r, g, b, 0.30];
    let accent_glow_core = [r, g, b, 0.80];
    let accent_tint = [r, g, b, 0.08];

    // Text hierarchy — UI foreground boosted toward white (dark) or black (light)
    // for WCAG AAA contrast on chrome surfaces. Terminal foreground stays unchanged.
    let (fr, fg_c, fb) = palette.foreground;
    let text_primary = ui_text_base(fr, fg_c, fb, l);
    let tp = text_primary;
    let text_secondary = [tp[0], tp[1], tp[2], 0.88];
    let text_muted = [tp[0], tp[1], tp[2], 0.75];
    let text_faint = [tp[0], tp[1], tp[2], 0.40];
    let text_inverse = [surface_base[0], surface_base[1], surface_base[2], 1.0];

    // Borders from surface_3
    let border_subtle = [surface_3[0], surface_3[1], surface_3[2], 0.40];
    let border_default = [surface_3[0], surface_3[1], surface_3[2], 0.60];
    let border_strong = [surface_3[0], surface_3[1], surface_3[2], 0.80];
    let border_glow = [r, g, b, 0.45];

    // Overlays
    let overlay_dim = [0.0, 0.0, 0.0, 0.50];
    let overlay_tint = [r, g, b, 0.08];

    // Semantic from ANSI colors
    let error = u8_to_f32_color(palette.ansi[1].0, palette.ansi[1].1, palette.ansi[1].2);
    let error_muted = [error[0], error[1], error[2], 0.12];
    let success = u8_to_f32_color(palette.ansi[2].0, palette.ansi[2].1, palette.ansi[2].2);
    let success_muted = [success[0], success[1], success[2], 0.12];
    let warning = u8_to_f32_color(palette.ansi[3].0, palette.ansi[3].1, palette.ansi[3].2);
    let warning_muted = [warning[0], warning[1], warning[2], 0.12];
    let info = accent;
    let info_muted = [r, g, b, 0.12];

    // Selection from palette.selection
    let (sr, sg, sb) = palette.selection;
    let sel = u8_to_f32_color(sr, sg, sb);
    let selection_bg = [sel[0], sel[1], sel[2], 0.30];

    // Search highlights from warning (ANSI yellow)
    let search_match = [warning[0], warning[1], warning[2], 0.30];
    let search_match_active = [warning[0], warning[1], warning[2], 0.50];

    // Drop shadow — black with moderate alpha
    let shadow_alpha = if l > 0.5 { 0.20 } else { 0.30 };
    let shadow = [0.0, 0.0, 0.0, shadow_alpha];

    // Shadow depth tokens (sigma, offset_y)
    let shadow_sm = ShadowDepth {
        sigma: 3.0,
        offset_y: 1.0,
    };
    let shadow_md = ShadowDepth {
        sigma: 5.0,
        offset_y: 2.0,
    };
    let shadow_lg = ShadowDepth {
        sigma: 8.0,
        offset_y: 4.0,
    };

    // Workspace dots from ANSI palette
    let dot_purple = u8_to_f32_color(palette.ansi[5].0, palette.ansi[5].1, palette.ansi[5].2);
    let dot_cyan = u8_to_f32_color(palette.ansi[6].0, palette.ansi[6].1, palette.ansi[6].2);

    // Cursor alpha
    let cursor_alpha = 0.85;

    UiChrome {
        surface_base,
        surface_0,
        surface_1,
        surface_2,
        surface_3,
        surface_overlay,
        accent,
        accent_hover,
        accent_muted,
        accent_glow,
        accent_glow_core,
        accent_tint,
        accent_pressed,
        text_primary,
        text_secondary,
        text_muted,
        text_faint,
        text_inverse,
        border_subtle,
        border_default,
        border_strong,
        border_glow,
        overlay_dim,
        overlay_tint,
        error,
        error_muted,
        success,
        success_muted,
        warning,
        warning_muted,
        info,
        info_muted,
        selection_bg,
        search_match,
        search_match_active,
        shadow,
        shadow_sm,
        shadow_md,
        shadow_lg,
        dot_purple,
        dot_cyan,
        cursor_alpha,
    }
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let hue_to_rgb = |t: f32| -> f32 {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };

    (
        hue_to_rgb(h + 1.0 / 3.0),
        hue_to_rgb(h),
        hue_to_rgb(h - 1.0 / 3.0),
    )
}

fn u8_to_f32_color(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// Compute the UI text base color by blending the theme foreground toward
/// white (dark themes) or black (light themes) for AAA contrast on chrome surfaces.
///
/// `bg_lightness` is the HSL lightness of the theme background (0.0–1.0).
fn ui_text_base(r: u8, g: u8, b: u8, bg_lightness: f32) -> [f32; 4] {
    let (fr, fg, fb) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let boost = 0.70;
    if bg_lightness < 0.5 {
        // Dark theme: blend 70% toward white
        [
            fr + (1.0 - fr) * boost,
            fg + (1.0 - fg) * boost,
            fb + (1.0 - fb) * boost,
            1.0,
        ]
    } else {
        // Light theme: blend 70% toward black
        [
            fr * (1.0 - boost),
            fg * (1.0 - boost),
            fb * (1.0 - boost),
            1.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeEngine;

    #[test]
    fn derive_ui_chrome_produces_valid_colors() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        // All color channels must be in 0.0..=1.0
        let all_colors = [
            chrome.surface_base,
            chrome.surface_0,
            chrome.surface_1,
            chrome.surface_2,
            chrome.surface_3,
            chrome.surface_overlay,
            chrome.accent,
            chrome.accent_hover,
            chrome.accent_muted,
            chrome.accent_glow,
            chrome.accent_glow_core,
            chrome.accent_tint,
            chrome.accent_pressed,
            chrome.text_primary,
            chrome.text_secondary,
            chrome.text_muted,
            chrome.text_faint,
            chrome.text_inverse,
            chrome.border_subtle,
            chrome.border_default,
            chrome.border_strong,
            chrome.border_glow,
            chrome.overlay_dim,
            chrome.overlay_tint,
            chrome.error,
            chrome.error_muted,
            chrome.success,
            chrome.success_muted,
            chrome.warning,
            chrome.warning_muted,
            chrome.info,
            chrome.info_muted,
            chrome.selection_bg,
            chrome.search_match,
            chrome.search_match_active,
            chrome.shadow,
            chrome.dot_purple,
            chrome.dot_cyan,
        ];
        for color in all_colors {
            for (i, &c) in color.iter().enumerate() {
                assert!(
                    (0.0..=1.0).contains(&c),
                    "channel {i} out of range: {c} in color {color:?}"
                );
            }
        }
    }

    #[test]
    fn derive_ui_chrome_surface_elevation_increases() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        // Each surface level must have increasing luminance (R+G+B sum as proxy)
        let lum = |c: [f32; 4]| c[0] + c[1] + c[2];
        assert!(
            lum(chrome.surface_0) > lum(chrome.surface_base),
            "surface_0 ({}) must be lighter than surface_base ({})",
            lum(chrome.surface_0),
            lum(chrome.surface_base)
        );
        assert!(
            lum(chrome.surface_1) > lum(chrome.surface_0),
            "surface_1 ({}) must be lighter than surface_0 ({})",
            lum(chrome.surface_1),
            lum(chrome.surface_0)
        );
        assert!(
            lum(chrome.surface_2) > lum(chrome.surface_1),
            "surface_2 ({}) must be lighter than surface_1 ({})",
            lum(chrome.surface_2),
            lum(chrome.surface_1)
        );
        assert!(
            lum(chrome.surface_3) > lum(chrome.surface_2),
            "surface_3 ({}) must be lighter than surface_2 ({})",
            lum(chrome.surface_3),
            lum(chrome.surface_2)
        );
    }

    #[test]
    fn derive_ui_chrome_accent_variants_have_correct_alpha() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        assert!((chrome.accent[3] - 1.0).abs() < f32::EPSILON);
        assert!((chrome.accent_hover[3] - 1.0).abs() < f32::EPSILON);
        assert!((chrome.accent_muted[3] - 0.30).abs() < f32::EPSILON);
        assert!((chrome.accent_glow[3] - 0.30).abs() < f32::EPSILON);
        assert!((chrome.accent_glow_core[3] - 0.80).abs() < f32::EPSILON);
        assert!((chrome.accent_tint[3] - 0.08).abs() < f32::EPSILON);
        // RGB channels must match for alpha variants
        for variant in [
            chrome.accent_muted,
            chrome.accent_glow,
            chrome.accent_glow_core,
            chrome.accent_tint,
        ] {
            assert_eq!(chrome.accent[0], variant[0]);
            assert_eq!(chrome.accent[1], variant[1]);
            assert_eq!(chrome.accent[2], variant[2]);
        }
    }

    #[test]
    fn derive_ui_chrome_text_hierarchy_alphas() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        assert!((chrome.text_primary[3] - 1.0).abs() < f32::EPSILON);
        assert!((chrome.text_secondary[3] - 0.88).abs() < f32::EPSILON);
        assert!((chrome.text_muted[3] - 0.75).abs() < f32::EPSILON);
        assert!((chrome.text_faint[3] - 0.40).abs() < f32::EPSILON);
        assert!((chrome.text_inverse[3] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn derive_ui_chrome_semantic_muted_variants() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        for (full, muted) in [
            (chrome.error, chrome.error_muted),
            (chrome.success, chrome.success_muted),
            (chrome.warning, chrome.warning_muted),
        ] {
            assert_eq!(full[0], muted[0]);
            assert_eq!(full[1], muted[1]);
            assert_eq!(full[2], muted[2]);
            assert!((muted[3] - 0.12).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn derive_ui_chrome_border_variants() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        assert!((chrome.border_subtle[3] - 0.40).abs() < f32::EPSILON);
        assert!((chrome.border_default[3] - 0.60).abs() < f32::EPSILON);
        assert!((chrome.border_strong[3] - 0.80).abs() < f32::EPSILON);
        assert!((chrome.border_glow[3] - 0.45).abs() < f32::EPSILON);
    }

    #[test]
    fn derive_ui_chrome_all_bundled_themes() {
        let engine = ThemeEngine::new();
        for name in engine.list_themes() {
            let theme = engine.load_theme(&name).unwrap();
            let chrome = derive_ui_chrome(&theme.palette);

            // Surface base must match background
            let (r, g, b) = theme.palette.background;
            let expected = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
            assert!(
                (chrome.surface_base[0] - expected[0]).abs() < 0.001
                    && (chrome.surface_base[1] - expected[1]).abs() < 0.001
                    && (chrome.surface_base[2] - expected[2]).abs() < 0.001,
                "theme '{name}': surface_base doesn't match background"
            );

            // Text primary must be boosted (brighter than raw foreground for dark themes)
            let (r, g, b) = theme.palette.foreground;
            let raw_lum = r as f32 + g as f32 + b as f32;
            let chrome_lum =
                chrome.text_primary[0] + chrome.text_primary[1] + chrome.text_primary[2];
            let (_, _, bg_l) = super::rgb_to_hsl(
                theme.palette.background.0 as f32 / 255.0,
                theme.palette.background.1 as f32 / 255.0,
                theme.palette.background.2 as f32 / 255.0,
            );
            if bg_l < 0.5 {
                assert!(
                    chrome_lum * 255.0 >= raw_lum - 1.0,
                    "theme '{name}': text_primary ({chrome_lum:.3}) must be >= foreground ({raw_lum:.0})"
                );
            } else {
                assert!(
                    chrome_lum * 255.0 <= raw_lum + 1.0,
                    "theme '{name}': text_primary ({chrome_lum:.3}) must be <= foreground ({raw_lum:.0})"
                );
            }
        }
    }

    #[test]
    fn derive_ui_chrome_text_boost_math() {
        // digital-obsidian: foreground #e2e2e2, bg L≈0.075 (dark)
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        // Boosted: fg + (1.0 - fg) * 0.70
        let fg = palette.foreground.0 as f32 / 255.0;
        let expected = fg + (1.0 - fg) * 0.70;
        assert!(
            (chrome.text_primary[0] - expected).abs() < 0.002,
            "text_primary R={}, expected ~{expected}",
            chrome.text_primary[0]
        );
        // Achromatic foreground → equal R/G/B
        assert!(
            (chrome.text_primary[0] - chrome.text_primary[1]).abs() < 0.001,
            "achromatic foreground should produce equal R/G/B"
        );
        assert!(
            (chrome.text_primary[1] - chrome.text_primary[2]).abs() < 0.001,
            "achromatic foreground should produce equal R/G/B"
        );
    }

    #[test]
    fn derive_ui_chrome_visual_pipeline_fields() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        // selection_bg derives from palette.selection at 30% alpha
        let (sr, sg, sb) = palette.selection;
        let expected_sel = [sr as f32 / 255.0, sg as f32 / 255.0, sb as f32 / 255.0];
        assert!((chrome.selection_bg[0] - expected_sel[0]).abs() < 0.001);
        assert!((chrome.selection_bg[1] - expected_sel[1]).abs() < 0.001);
        assert!((chrome.selection_bg[2] - expected_sel[2]).abs() < 0.001);
        assert!((chrome.selection_bg[3] - 0.30).abs() < f32::EPSILON);

        // search_match and search_match_active derive from warning
        assert_eq!(chrome.search_match[0], chrome.warning[0]);
        assert_eq!(chrome.search_match[1], chrome.warning[1]);
        assert_eq!(chrome.search_match[2], chrome.warning[2]);
        assert!((chrome.search_match[3] - 0.30).abs() < f32::EPSILON);
        assert!((chrome.search_match_active[3] - 0.50).abs() < f32::EPSILON);

        // shadow is black with alpha
        assert!((chrome.shadow[0]).abs() < f32::EPSILON);
        assert!((chrome.shadow[1]).abs() < f32::EPSILON);
        assert!((chrome.shadow[2]).abs() < f32::EPSILON);
        assert!(chrome.shadow[3] > 0.0 && chrome.shadow[3] <= 0.50);

        // cursor_alpha default
        assert!((chrome.cursor_alpha - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn hsl_roundtrip_preserves_color() {
        let test_cases = [
            (0.5, 0.5, 0.5),    // mid gray
            (1.0, 0.0, 0.0),    // red
            (0.0, 1.0, 0.0),    // green
            (0.0, 0.0, 1.0),    // blue
            (0.12, 0.12, 0.14), // wmux-default bg approx
        ];

        for (r, g, b) in test_cases {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (r2, g2, b2) = hsl_to_rgb(h, s, l);
            assert!(
                (r - r2).abs() < 0.01 && (g - g2).abs() < 0.01 && (b - b2).abs() < 0.01,
                "roundtrip failed for ({r}, {g}, {b}) → HSL({h}, {s}, {l}) → ({r2}, {g2}, {b2})"
            );
        }
    }
}
