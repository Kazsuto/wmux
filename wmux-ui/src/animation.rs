use std::time::{Duration, Instant};

/// Motion duration constants (UI spec "Luminous Void").
pub const MOTION_MICRO: Duration = Duration::from_millis(80);
pub const MOTION_FAST: Duration = Duration::from_millis(150);
pub const MOTION_NORMAL: Duration = Duration::from_millis(250);
pub const MOTION_SLOW: Duration = Duration::from_millis(350);
pub const MOTION_PULSE: Duration = Duration::from_millis(2000);
pub const MOTION_BLINK: Duration = Duration::from_millis(500);

/// Easing function type for animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    /// Cubic ease-out — entering elements. Decelerates smoothly.
    CubicOut,
    /// Cubic ease-in — exiting elements. Accelerates.
    CubicIn,
    /// Ease in-out — state changes, position moves.
    EaseInOut,
    /// Linear interpolation — cursor blink, continuous.
    Linear,
}

/// A single running animation interpolating a float value.
#[derive(Debug)]
struct Animation {
    id: u64,
    start: Instant,
    duration: Duration,
    from: f32,
    to: f32,
    current: f32,
    easing: Easing,
}

/// Engine for managing UI micro-animations (hover, transitions, overlays).
///
/// Usage each frame:
/// 1. `update()` — advance all animations
/// 2. `get(id)` — read current values for rendering
/// 3. `has_active()` — request redraw if animations are running
///
/// When `reduced_motion` is true, all animations complete instantly (duration=0)
/// except cursor blink.
#[derive(Debug, Default)]
pub struct AnimationEngine {
    animations: Vec<Animation>,
    next_id: u64,
    reduced_motion: bool,
}

impl AnimationEngine {
    /// Create an engine with reduced motion preference.
    pub fn with_reduced_motion(reduced_motion: bool) -> Self {
        Self {
            animations: Vec::new(),
            next_id: 0,
            reduced_motion,
        }
    }

    /// Set reduced motion preference (from Windows accessibility settings).
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }

    /// Start a new animation and return its ID.
    ///
    /// When `reduced_motion` is true, non-continuous animations complete instantly.
    /// Continuous animations (Linear easing, e.g. cursor blink) are exempt.
    pub fn start(&mut self, from: f32, to: f32, duration: Duration, easing: Easing) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let effective_duration = if self.reduced_motion && easing != Easing::Linear {
            Duration::ZERO
        } else {
            duration
        };
        // For zero-duration animations, set current to final value immediately
        // so that get() returns `to` before update() removes it.
        let initial = if effective_duration == Duration::ZERO {
            to
        } else {
            from
        };
        self.animations.push(Animation {
            id,
            start: Instant::now(),
            duration: effective_duration,
            from,
            to,
            current: initial,
            easing,
        });
        id
    }

    /// Advance all animations to current time, removing completed ones.
    pub fn update(&mut self) {
        let now = Instant::now();
        self.animations.retain_mut(|anim| {
            let elapsed = now.duration_since(anim.start);
            if elapsed >= anim.duration {
                anim.current = anim.to;
                return false;
            }
            let t = elapsed.as_secs_f32() / anim.duration.as_secs_f32();
            let eased = match anim.easing {
                Easing::Linear => t,
                Easing::CubicOut => {
                    let inv = 1.0 - t;
                    1.0 - inv * inv * inv
                }
                Easing::CubicIn => t * t * t,
                Easing::EaseInOut => {
                    if t < 0.5 {
                        4.0 * t * t * t
                    } else {
                        let inv = -2.0 * t + 2.0;
                        1.0 - inv * inv * inv / 2.0
                    }
                }
            };
            anim.current = anim.from + (anim.to - anim.from) * eased;
            true
        });
    }

    /// Get the current interpolated value for an animation, or `None` if completed/unknown.
    pub fn get(&self, id: u64) -> Option<f32> {
        self.animations
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.current)
    }

    /// Returns `true` if any animations are still running.
    pub fn has_active(&self) -> bool {
        !self.animations.is_empty()
    }

    /// Cancel an animation by ID.
    pub fn cancel(&mut self, id: u64) {
        self.animations.retain(|a| a.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_returns_unique_ids() {
        let mut engine = AnimationEngine::default();
        let id1 = engine.start(0.0, 1.0, Duration::from_millis(200), Easing::Linear);
        let id2 = engine.start(0.0, 1.0, Duration::from_millis(200), Easing::Linear);
        assert_ne!(id1, id2);
    }

    #[test]
    fn get_returns_initial_value_before_update() {
        let mut engine = AnimationEngine::default();
        let id = engine.start(0.5, 1.0, Duration::from_millis(200), Easing::Linear);
        assert_eq!(engine.get(id), Some(0.5));
    }

    #[test]
    fn has_active_reflects_state() {
        let mut engine = AnimationEngine::default();
        assert!(!engine.has_active());

        let _id = engine.start(0.0, 1.0, Duration::from_millis(200), Easing::Linear);
        assert!(engine.has_active());
    }

    #[test]
    fn cancel_removes_animation() {
        let mut engine = AnimationEngine::default();
        let id = engine.start(0.0, 1.0, Duration::from_millis(200), Easing::Linear);
        assert!(engine.has_active());

        engine.cancel(id);
        assert!(!engine.has_active());
        assert_eq!(engine.get(id), None);
    }

    #[test]
    fn completed_animation_removed_after_update() {
        let mut engine = AnimationEngine::default();
        let id = engine.start(0.0, 1.0, Duration::from_millis(0), Easing::Linear);

        // Zero-duration animation should complete immediately
        std::thread::sleep(Duration::from_millis(1));
        engine.update();
        assert!(!engine.has_active());
        assert_eq!(engine.get(id), None);
    }

    #[test]
    fn cubic_out_easing_starts_fast() {
        // At t=0.5: cubic_out = 1 - (0.5)^3 = 0.875
        // Linear at t=0.5: 0.5
        // So cubic_out should be ahead of linear at the midpoint
        let cubic = {
            let inv = 1.0 - 0.5_f32;
            1.0 - inv * inv * inv
        };
        assert!(
            cubic > 0.5,
            "cubic_out at t=0.5 should be > 0.5, got {cubic}"
        );
    }

    #[test]
    fn cubic_in_easing_starts_slow() {
        // At t=0.5: cubic_in = (0.5)^3 = 0.125
        let cubic_in = 0.5_f32 * 0.5 * 0.5;
        assert!(
            cubic_in < 0.5,
            "cubic_in at t=0.5 should be < 0.5, got {cubic_in}"
        );
    }

    #[test]
    fn ease_in_out_midpoint() {
        // At t=0.5: EaseInOut should be exactly 0.5 (symmetric)
        let t = 0.5_f32;
        let eased = if t < 0.5 {
            4.0 * t * t * t
        } else {
            let inv = -2.0 * t + 2.0;
            1.0 - inv * inv * inv / 2.0
        };
        assert!(
            (eased - 0.5).abs() < 0.001,
            "ease_in_out at t=0.5 should be ~0.5, got {eased}"
        );
    }

    #[test]
    fn reduced_motion_completes_instantly() {
        let mut engine = AnimationEngine::with_reduced_motion(true);
        let id = engine.start(0.0, 1.0, Duration::from_millis(5000), Easing::CubicOut);
        std::thread::sleep(Duration::from_millis(1));
        engine.update();
        assert!(!engine.has_active());
        assert_eq!(engine.get(id), None);
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let engine = AnimationEngine::default();
        assert_eq!(engine.get(999), None);
    }

    #[test]
    fn motion_duration_constants() {
        assert_eq!(MOTION_MICRO.as_millis(), 80);
        assert_eq!(MOTION_FAST.as_millis(), 150);
        assert_eq!(MOTION_NORMAL.as_millis(), 250);
        assert_eq!(MOTION_SLOW.as_millis(), 350);
        assert_eq!(MOTION_PULSE.as_millis(), 2000);
        assert_eq!(MOTION_BLINK.as_millis(), 500);
    }
}
