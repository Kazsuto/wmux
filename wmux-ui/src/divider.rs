use wmux_core::{rect::Rect, PaneId};

/// Direction of the divider (matches the split axis).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerOrientation {
    /// Split is horizontal → divider is a vertical bar between left/right panes.
    Vertical,
    /// Split is vertical → divider is a horizontal bar between top/bottom panes.
    Horizontal,
}

/// A detected divider between two panes.
#[derive(Debug, Clone)]
pub struct Divider {
    pub orientation: DividerOrientation,
    /// The pane ID used for resize_split (second child of the split node).
    pub pane_id: PaneId,
    /// Divider center position (x for vertical, y for horizontal).
    pub position: f32,
    /// Range on the perpendicular axis (start coordinate).
    pub start: f32,
    /// Range on the perpendicular axis (end coordinate).
    pub end: f32,
}

/// Tracks active divider drag state.
#[derive(Debug, Clone)]
pub struct DragState {
    pub pane_id: PaneId,
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

/// Find all dividers from a flat layout (pane_id, rect) slice.
///
/// Iterates all pairs of panes and detects shared vertical or horizontal edges.
#[must_use]
pub fn find_dividers(layout: &[(PaneId, Rect)]) -> Vec<Divider> {
    let mut dividers = Vec::new();

    for i in 0..layout.len() {
        for j in (i + 1)..layout.len() {
            let (id_a, a) = &layout[i];
            let (id_b, b) = &layout[j];

            let a_right = a.x + a.width;
            let b_right = b.x + b.width;

            // Check vertical edge: a is left of b
            if (a_right - b.x).abs() < 2.0 {
                let overlap_start = a.y.max(b.y);
                let overlap_end = (a.y + a.height).min(b.y + b.height);
                if overlap_end > overlap_start {
                    dividers.push(Divider {
                        orientation: DividerOrientation::Vertical,
                        pane_id: *id_b,
                        position: a_right,
                        start: overlap_start,
                        end: overlap_end,
                    });
                }
            // Check vertical edge: b is left of a
            } else if (b_right - a.x).abs() < 2.0 {
                let overlap_start = a.y.max(b.y);
                let overlap_end = (a.y + a.height).min(b.y + b.height);
                if overlap_end > overlap_start {
                    dividers.push(Divider {
                        orientation: DividerOrientation::Vertical,
                        pane_id: *id_a,
                        position: b_right,
                        start: overlap_start,
                        end: overlap_end,
                    });
                }
            }

            let a_bottom = a.y + a.height;
            let b_bottom = b.y + b.height;

            // Check horizontal edge: a is above b
            if (a_bottom - b.y).abs() < 2.0 {
                let overlap_start = a.x.max(b.x);
                let overlap_end = (a.x + a.width).min(b.x + b.width);
                if overlap_end > overlap_start {
                    dividers.push(Divider {
                        orientation: DividerOrientation::Horizontal,
                        pane_id: *id_b,
                        position: a_bottom,
                        start: overlap_start,
                        end: overlap_end,
                    });
                }
            // Check horizontal edge: b is above a
            } else if (b_bottom - a.y).abs() < 2.0 {
                let overlap_start = a.x.max(b.x);
                let overlap_end = (a.x + a.width).min(b.x + b.width);
                if overlap_end > overlap_start {
                    dividers.push(Divider {
                        orientation: DividerOrientation::Horizontal,
                        pane_id: *id_a,
                        position: b_bottom,
                        start: overlap_start,
                        end: overlap_end,
                    });
                }
            }
        }
    }

    dividers
}

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
    raw_ratio.clamp(min_ratio, max_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_dividers_horizontal_split() {
        let id_a = PaneId::new();
        let id_b = PaneId::new();
        // a is on the left, b is on the right with a 4px gap
        let rect_a = Rect::new(0.0, 0.0, 100.0, 200.0);
        let rect_b = Rect::new(104.0, 0.0, 100.0, 200.0);
        // a_right = 100, b.x = 104 — gap of 4px, not adjacent by <2px threshold
        // Let's use adjacent rects (gap=0 or within 2px tolerance)
        let rect_b_adj = Rect::new(100.0, 0.0, 100.0, 200.0);

        let layout = vec![(id_a, rect_a), (id_b, rect_b_adj)];
        let dividers = find_dividers(&layout);

        assert_eq!(dividers.len(), 1);
        let div = &dividers[0];
        assert_eq!(div.orientation, DividerOrientation::Vertical);
        assert_eq!(div.pane_id, id_b);
        assert!((div.position - 100.0).abs() < 1e-4);
        assert_eq!(div.start, 0.0);
        assert_eq!(div.end, 200.0);
    }

    #[test]
    fn find_dividers_vertical_split() {
        let id_a = PaneId::new();
        let id_b = PaneId::new();
        let rect_a = Rect::new(0.0, 0.0, 200.0, 100.0);
        let rect_b = Rect::new(0.0, 100.0, 200.0, 100.0);

        let layout = vec![(id_a, rect_a), (id_b, rect_b)];
        let dividers = find_dividers(&layout);

        assert_eq!(dividers.len(), 1);
        let div = &dividers[0];
        assert_eq!(div.orientation, DividerOrientation::Horizontal);
        assert_eq!(div.pane_id, id_b);
        assert!((div.position - 100.0).abs() < 1e-4);
    }

    #[test]
    fn find_dividers_single_pane_no_dividers() {
        let id_a = PaneId::new();
        let layout = vec![(id_a, Rect::new(0.0, 0.0, 800.0, 600.0))];
        let dividers = find_dividers(&layout);
        assert!(dividers.is_empty());
    }

    #[test]
    fn hit_test_vertical_divider() {
        let id_a = PaneId::new();
        let id_b = PaneId::new();
        let layout = vec![
            (id_a, Rect::new(0.0, 0.0, 100.0, 200.0)),
            (id_b, Rect::new(100.0, 0.0, 100.0, 200.0)),
        ];
        let dividers = find_dividers(&layout);

        // Should hit at x=100, y=100 (center of divider range)
        let hit = hit_test(&dividers, 100.0, 100.0);
        assert!(hit.is_some());

        // Should miss far away
        let miss = hit_test(&dividers, 200.0, 100.0);
        assert!(miss.is_none());

        // Should miss outside perpendicular range
        let miss_y = hit_test(&dividers, 100.0, 250.0);
        assert!(miss_y.is_none());
    }

    #[test]
    fn hit_test_horizontal_divider() {
        let id_a = PaneId::new();
        let id_b = PaneId::new();
        let layout = vec![
            (id_a, Rect::new(0.0, 0.0, 200.0, 100.0)),
            (id_b, Rect::new(0.0, 100.0, 200.0, 100.0)),
        ];
        let dividers = find_dividers(&layout);

        let hit = hit_test(&dividers, 100.0, 100.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().orientation, DividerOrientation::Horizontal);
    }

    #[test]
    fn compute_ratio_clamps_minimum() {
        let id = PaneId::new();
        let drag = DragState {
            pane_id: id,
            orientation: DividerOrientation::Vertical,
            split_dimension: 400.0,
            split_start: 0.0,
            start_cursor: 200.0,
            start_ratio: 0.5,
        };
        // Drag far to the left — should clamp to MIN_PANE_SIZE / split_dimension
        let ratio = compute_ratio(&drag, -9999.0);
        let expected_min = MIN_PANE_SIZE / 400.0;
        assert!((ratio - expected_min).abs() < 1e-4);
    }

    #[test]
    fn compute_ratio_clamps_maximum() {
        let id = PaneId::new();
        let drag = DragState {
            pane_id: id,
            orientation: DividerOrientation::Vertical,
            split_dimension: 400.0,
            split_start: 0.0,
            start_cursor: 200.0,
            start_ratio: 0.5,
        };
        // Drag far to the right — should clamp to 1 - MIN_PANE_SIZE / split_dimension
        let ratio = compute_ratio(&drag, 9999.0);
        let expected_max = 1.0 - MIN_PANE_SIZE / 400.0;
        assert!((ratio - expected_max).abs() < 1e-4);
    }

    #[test]
    fn compute_ratio_no_movement() {
        let id = PaneId::new();
        let drag = DragState {
            pane_id: id,
            orientation: DividerOrientation::Horizontal,
            split_dimension: 600.0,
            split_start: 0.0,
            start_cursor: 300.0,
            start_ratio: 0.5,
        };
        // No drag delta — ratio stays at start_ratio
        let ratio = compute_ratio(&drag, 300.0);
        assert!((ratio - 0.5).abs() < 1e-4);
    }
}
