use wmux_core::{pane_tree::LayoutDivider, SplitDirection, SplitId};

/// Direction of the divider (matches the split axis).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerOrientation {
    /// Split is horizontal → divider is a vertical bar between left/right panes.
    Vertical,
    /// Split is vertical → divider is a horizontal bar between top/bottom panes.
    Horizontal,
}

impl DividerOrientation {
    /// Convert from `SplitDirection` (tree domain) to `DividerOrientation` (UI domain).
    #[must_use]
    pub fn from_split_direction(dir: SplitDirection) -> Self {
        match dir {
            SplitDirection::Horizontal => Self::Vertical,
            SplitDirection::Vertical => Self::Horizontal,
        }
    }
}

/// A detected divider between two panes.
#[derive(Debug, Clone)]
pub struct Divider {
    pub orientation: DividerOrientation,
    /// The split node ID used for `resize_by_split_id`.
    pub split_id: SplitId,
    /// Divider center position (x for vertical, y for horizontal).
    pub position: f32,
    /// Range on the perpendicular axis (start coordinate).
    pub start: f32,
    /// Range on the perpendicular axis (end coordinate).
    pub end: f32,
    /// Offset of the split container's origin on the split axis.
    pub split_start: f32,
    /// Total dimension of the split container on the split axis.
    pub split_dimension: f32,
    /// Current ratio of the split node.
    pub current_ratio: f32,
}

impl From<LayoutDivider> for Divider {
    fn from(ld: LayoutDivider) -> Self {
        Self {
            orientation: DividerOrientation::from_split_direction(ld.direction),
            split_id: ld.split_id,
            position: ld.position,
            start: ld.start,
            end: ld.end,
            split_start: ld.split_start,
            split_dimension: ld.split_dimension,
            current_ratio: ld.current_ratio,
        }
    }
}

/// Tracks active divider drag state.
#[derive(Debug, Clone)]
pub struct DragState {
    pub split_id: SplitId,
    pub orientation: DividerOrientation,
    /// Total split dimension (width for vertical divider, height for horizontal).
    pub split_dimension: f32,
    /// Offset of the split start (x for vertical, y for horizontal).
    pub split_start: f32,
    /// Cursor position when the drag started.
    pub start_cursor: f32,
    /// Split ratio when the drag started.
    pub start_ratio: f32,
}

/// Hit zone radius (4px gap + 2px padding on each side = 8px total).
const HIT_ZONE: f32 = 8.0;

/// Minimum pane size in pixels (prevent collapse during drag).
pub const MIN_PANE_SIZE: f32 = 50.0;

/// Hit-test a cursor position against a list of dividers.
///
/// Returns the first divider whose hit zone contains the point, or `None`.
#[must_use]
pub fn hit_test(dividers: &[Divider], x: f32, y: f32) -> Option<&Divider> {
    for div in dividers {
        match div.orientation {
            DividerOrientation::Vertical => {
                if (x - div.position).abs() < HIT_ZONE / 2.0 && y >= div.start && y <= div.end {
                    return Some(div);
                }
            }
            DividerOrientation::Horizontal => {
                if (y - div.position).abs() < HIT_ZONE / 2.0 && x >= div.start && x <= div.end {
                    return Some(div);
                }
            }
        }
    }
    None
}

/// Compute the new split ratio given a drag state and the current cursor position.
///
/// Clamps the result so neither pane falls below `MIN_PANE_SIZE`.
#[must_use]
pub fn compute_ratio(drag: &DragState, cursor: f32) -> f32 {
    if drag.split_dimension <= 0.0 {
        return 0.5;
    }
    let delta = cursor - drag.start_cursor;
    let new_position = (drag.split_start + drag.split_dimension * drag.start_ratio) + delta;
    let raw_ratio = (new_position - drag.split_start) / drag.split_dimension;
    let min_ratio = MIN_PANE_SIZE / drag.split_dimension;
    let max_ratio = 1.0 - min_ratio;
    if min_ratio >= max_ratio {
        // Split too small for two panes — keep centered.
        return 0.5;
    }
    raw_ratio.clamp(min_ratio, max_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_divider(
        orientation: DividerOrientation,
        position: f32,
        start: f32,
        end: f32,
    ) -> Divider {
        Divider {
            orientation,
            split_id: SplitId::new(),
            position,
            start,
            end,
            split_start: 0.0,
            split_dimension: 400.0,
            current_ratio: 0.5,
        }
    }

    #[test]
    fn hit_test_vertical_divider() {
        let dividers = vec![make_divider(
            DividerOrientation::Vertical,
            100.0,
            0.0,
            200.0,
        )];

        let hit = hit_test(&dividers, 100.0, 100.0);
        assert!(hit.is_some());

        let miss = hit_test(&dividers, 200.0, 100.0);
        assert!(miss.is_none());

        let miss_y = hit_test(&dividers, 100.0, 250.0);
        assert!(miss_y.is_none());
    }

    #[test]
    fn hit_test_horizontal_divider() {
        let dividers = vec![make_divider(
            DividerOrientation::Horizontal,
            100.0,
            0.0,
            200.0,
        )];

        let hit = hit_test(&dividers, 100.0, 100.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().orientation, DividerOrientation::Horizontal);
    }

    #[test]
    fn compute_ratio_clamps_minimum() {
        let drag = DragState {
            split_id: SplitId::new(),
            orientation: DividerOrientation::Vertical,
            split_dimension: 400.0,
            split_start: 0.0,
            start_cursor: 200.0,
            start_ratio: 0.5,
        };
        let ratio = compute_ratio(&drag, -9999.0);
        let expected_min = MIN_PANE_SIZE / 400.0;
        assert!((ratio - expected_min).abs() < 1e-4);
    }

    #[test]
    fn compute_ratio_clamps_maximum() {
        let drag = DragState {
            split_id: SplitId::new(),
            orientation: DividerOrientation::Vertical,
            split_dimension: 400.0,
            split_start: 0.0,
            start_cursor: 200.0,
            start_ratio: 0.5,
        };
        let ratio = compute_ratio(&drag, 9999.0);
        let expected_max = 1.0 - MIN_PANE_SIZE / 400.0;
        assert!((ratio - expected_max).abs() < 1e-4);
    }

    #[test]
    fn compute_ratio_no_movement() {
        let drag = DragState {
            split_id: SplitId::new(),
            orientation: DividerOrientation::Horizontal,
            split_dimension: 600.0,
            split_start: 0.0,
            start_cursor: 300.0,
            start_ratio: 0.5,
        };
        let ratio = compute_ratio(&drag, 300.0);
        assert!((ratio - 0.5).abs() < 1e-4);
    }

    #[test]
    fn layout_divider_conversion() {
        let ld = LayoutDivider {
            direction: SplitDirection::Horizontal,
            split_id: SplitId::new(),
            position: 500.0,
            start: 0.0,
            end: 800.0,
            split_start: 0.0,
            split_dimension: 1000.0,
            current_ratio: 0.5,
        };
        let div = Divider::from(ld);
        assert_eq!(div.orientation, DividerOrientation::Vertical);
        assert!((div.position - 500.0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_ratio_small_split_no_panic() {
        // split_dimension < 2 * MIN_PANE_SIZE → min_ratio > max_ratio.
        // Previously this would panic in f32::clamp.
        let drag = DragState {
            split_id: SplitId::new(),
            orientation: DividerOrientation::Vertical,
            split_dimension: 80.0, // < 2 * MIN_PANE_SIZE (100)
            split_start: 0.0,
            start_cursor: 40.0,
            start_ratio: 0.5,
        };
        let ratio = compute_ratio(&drag, 60.0);
        assert!((ratio - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn divider_orientation_from_split_direction() {
        assert_eq!(
            DividerOrientation::from_split_direction(SplitDirection::Horizontal),
            DividerOrientation::Vertical
        );
        assert_eq!(
            DividerOrientation::from_split_direction(SplitDirection::Vertical),
            DividerOrientation::Horizontal
        );
    }
}
