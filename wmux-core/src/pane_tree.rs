use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::rect::{Rect, DIVIDER_WIDTH};
use crate::surface::SplitDirection;
use crate::types::{PaneId, SplitId};

/// Divider metadata emitted during tree layout traversal.
///
/// Each `LayoutDivider` corresponds to exactly one `Split` node in the
/// `PaneTree`. It carries the split's `SplitId` so the UI can resize the
/// correct node when the user drags the divider.
#[derive(Debug, Clone)]
pub struct LayoutDivider {
    /// Direction of the split that produced this divider.
    pub direction: SplitDirection,
    /// Unique identifier of the split node (for resize).
    pub split_id: SplitId,
    /// Divider center position (x for vertical bar, y for horizontal bar).
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

/// Binary tree representing pane layout within a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneTree {
    /// A leaf node containing a single pane.
    Leaf(PaneId),
    /// A split node dividing space between two children.
    Split {
        /// Unique identifier for this split node (for targeted resize).
        #[serde(default = "SplitId::new")]
        split_id: SplitId,
        direction: SplitDirection,
        /// Fraction of space allocated to the first child (0.0--1.0).
        ratio: f32,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}

impl PaneTree {
    /// Create a new tree with a single leaf pane.
    #[must_use]
    pub fn new(pane_id: PaneId) -> Self {
        Self::Leaf(pane_id)
    }

    /// Recursively compute the layout, returning `(PaneId, Rect)` pairs for
    /// every leaf in the tree.
    #[must_use]
    pub fn layout(&self, viewport: Rect) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::with_capacity(self.pane_count());
        self.layout_into(viewport, &mut out);
        out
    }

    /// Accumulate layout results into a pre-allocated buffer (avoids
    /// intermediate allocations in recursive calls).
    fn layout_into(&self, viewport: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match self {
            Self::Leaf(id) => out.push((*id, viewport)),
            Self::Split {
                direction,
                ratio,
                first,
                second,
                ..
            } => {
                let (first_rect, second_rect) = match direction {
                    SplitDirection::Horizontal => viewport.split_horizontal(*ratio, DIVIDER_WIDTH),
                    SplitDirection::Vertical => viewport.split_vertical(*ratio, DIVIDER_WIDTH),
                };
                first.layout_into(first_rect, out);
                second.layout_into(second_rect, out);
            }
        }
    }

    /// Compute layout AND collect divider metadata from the tree structure.
    ///
    /// Unlike [`layout`] + flat-layout divider detection, this method
    /// directly associates each divider with its originating `Split` node,
    /// guaranteeing correct resize targeting in nested trees.
    #[must_use]
    pub fn layout_with_dividers(
        &self,
        viewport: Rect,
    ) -> (Vec<(PaneId, Rect)>, Vec<LayoutDivider>) {
        let n = self.pane_count();
        let mut panes = Vec::with_capacity(n);
        let mut dividers = Vec::with_capacity(n.saturating_sub(1));
        self.layout_dividers_into(viewport, &mut panes, &mut dividers);
        (panes, dividers)
    }

    fn layout_dividers_into(
        &self,
        viewport: Rect,
        panes: &mut Vec<(PaneId, Rect)>,
        dividers: &mut Vec<LayoutDivider>,
    ) {
        match self {
            Self::Leaf(id) => panes.push((*id, viewport)),
            Self::Split {
                split_id,
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_rect, second_rect) = match direction {
                    SplitDirection::Horizontal => viewport.split_horizontal(*ratio, DIVIDER_WIDTH),
                    SplitDirection::Vertical => viewport.split_vertical(*ratio, DIVIDER_WIDTH),
                };

                // Emit divider at the boundary between the two children.
                let (position, start, end, split_start, split_dimension) = match direction {
                    SplitDirection::Horizontal => (
                        first_rect.x + first_rect.width + DIVIDER_WIDTH / 2.0,
                        viewport.y,
                        viewport.y + viewport.height,
                        viewport.x,
                        viewport.width,
                    ),
                    SplitDirection::Vertical => (
                        first_rect.y + first_rect.height + DIVIDER_WIDTH / 2.0,
                        viewport.x,
                        viewport.x + viewport.width,
                        viewport.y,
                        viewport.height,
                    ),
                };

                dividers.push(LayoutDivider {
                    direction: *direction,
                    split_id: *split_id,
                    position,
                    start,
                    end,
                    split_start,
                    split_dimension,
                    current_ratio: *ratio,
                });

                first.layout_dividers_into(first_rect, panes, dividers);
                second.layout_dividers_into(second_rect, panes, dividers);
            }
        }
    }

    /// Split the leaf identified by `target` into two panes along `direction`.
    ///
    /// The existing leaf becomes the first child and a new leaf (with a fresh
    /// `PaneId`) becomes the second child. Returns the new pane's ID.
    ///
    /// **Note:** The caller is responsible for registering the new pane in the
    /// `PaneRegistry` (spawning a PTY, creating a terminal, etc.). This method
    /// only updates the layout tree.
    pub fn split_pane(
        &mut self,
        target: PaneId,
        direction: SplitDirection,
    ) -> Result<PaneId, CoreError> {
        let new_id = PaneId::new();
        if self.split_pane_inner(target, direction, new_id) {
            Ok(new_id)
        } else {
            Err(CoreError::PaneNotFound {
                pane_id: target.to_string(),
            })
        }
    }

    /// Recursive helper that performs the actual split mutation.
    /// Returns `true` if the target was found and split.
    fn split_pane_inner(
        &mut self,
        target: PaneId,
        direction: SplitDirection,
        new_id: PaneId,
    ) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                let old = Self::Leaf(*id);
                *self = Self::Split {
                    split_id: SplitId::new(),
                    direction,
                    ratio: 0.5,
                    first: Box::new(old),
                    second: Box::new(Self::Leaf(new_id)),
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                first.split_pane_inner(target, direction, new_id)
                    || second.split_pane_inner(target, direction, new_id)
            }
        }
    }

    /// Close (remove) the leaf identified by `target`.
    ///
    /// The parent split is replaced by the sibling subtree. Returns an error
    /// if the target is the last remaining pane or is not found.
    pub fn close_pane(&mut self, target: PaneId) -> Result<(), CoreError> {
        // Cannot close the last pane.
        if matches!(self, Self::Leaf(id) if *id == target) {
            return Err(CoreError::CannotClose("cannot close last pane".to_string()));
        }

        if !self.find_pane(target) {
            return Err(CoreError::PaneNotFound {
                pane_id: target.to_string(),
            });
        }

        if let Some(replacement) = self.close_pane_inner(target) {
            *self = replacement;
        }
        Ok(())
    }

    /// Recursive helper. Returns `Some(sibling)` when the target was found
    /// inside this node and the node should be replaced by the sibling.
    /// Returns `None` when the target was found deeper (already handled).
    fn close_pane_inner(&mut self, target: PaneId) -> Option<PaneTree> {
        match self {
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => {
                // Check if first child IS the target leaf — promote second.
                if matches!(first.as_ref(), Self::Leaf(id) if *id == target) {
                    // Take ownership of the sibling via mem::replace (no clone).
                    let placeholder = Self::Leaf(PaneId::default());
                    let sibling = std::mem::replace(second.as_mut(), placeholder);
                    return Some(sibling);
                }
                // Check if second child IS the target leaf — promote first.
                if matches!(second.as_ref(), Self::Leaf(id) if *id == target) {
                    let placeholder = Self::Leaf(PaneId::default());
                    let sibling = std::mem::replace(first.as_mut(), placeholder);
                    return Some(sibling);
                }
                // Recurse into children.
                if let Some(replacement) = first.close_pane_inner(target) {
                    **first = replacement;
                    return None;
                }
                if let Some(replacement) = second.close_pane_inner(target) {
                    **second = replacement;
                    return None;
                }
                None
            }
        }
    }

    /// Swap the positions of two panes identified by their IDs.
    ///
    /// Uses a single-pass traversal — no sentinel values.
    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), CoreError> {
        if !self.find_pane(a) {
            return Err(CoreError::PaneNotFound {
                pane_id: a.to_string(),
            });
        }
        if !self.find_pane(b) {
            return Err(CoreError::PaneNotFound {
                pane_id: b.to_string(),
            });
        }
        if a == b {
            return Ok(());
        }
        self.swap_ids(a, b);
        Ok(())
    }

    /// Single-pass swap: visit every leaf, exchange a↔b directly.
    fn swap_ids(&mut self, a: PaneId, b: PaneId) {
        match self {
            Self::Leaf(id) if *id == a => *id = b,
            Self::Leaf(id) if *id == b => *id = a,
            Self::Leaf(_) => {}
            Self::Split { first, second, .. } => {
                first.swap_ids(a, b);
                second.swap_ids(a, b);
            }
        }
    }

    /// Resize the split that is the immediate parent of `pane_id`, setting
    /// its ratio to `new_ratio` (clamped to 0.1..=0.9).
    pub fn resize_split(&mut self, pane_id: PaneId, new_ratio: f32) -> Result<(), CoreError> {
        if !self.find_pane(pane_id) {
            return Err(CoreError::PaneNotFound {
                pane_id: pane_id.to_string(),
            });
        }
        if !self.resize_split_inner(pane_id, new_ratio) {
            return Err(CoreError::CannotSplit(
                "pane has no parent split to resize".to_string(),
            ));
        }
        Ok(())
    }

    /// Recursive helper. Returns `true` if a parent split was found and resized.
    ///
    /// Only resizes the immediate parent — checks direct children (not deep).
    fn resize_split_inner(&mut self, pane_id: PaneId, new_ratio: f32) -> bool {
        match self {
            Self::Leaf(_) => false,
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                // Check only DIRECT children (immediate parent detection).
                let first_is_target = matches!(first.as_ref(), Self::Leaf(id) if *id == pane_id);
                let second_is_target = matches!(second.as_ref(), Self::Leaf(id) if *id == pane_id);
                if first_is_target || second_is_target {
                    *ratio = new_ratio.clamp(0.1, 0.9);
                    return true;
                }
                // Recurse deeper.
                first.resize_split_inner(pane_id, new_ratio)
                    || second.resize_split_inner(pane_id, new_ratio)
            }
        }
    }

    /// Resize a specific split node by its `SplitId`.
    ///
    /// Unlike [`resize_split`] (which walks by pane ID and may hit the wrong
    /// node in nested trees), this method targets the exact split node.
    pub fn resize_by_split_id(&mut self, target: SplitId, new_ratio: f32) -> Result<(), CoreError> {
        if self.resize_by_split_id_inner(target, new_ratio) {
            Ok(())
        } else {
            Err(CoreError::CannotSplit("split node not found".to_string()))
        }
    }

    fn resize_by_split_id_inner(&mut self, target: SplitId, new_ratio: f32) -> bool {
        match self {
            Self::Leaf(_) => false,
            Self::Split {
                split_id,
                ratio,
                first,
                second,
                ..
            } => {
                if *split_id == target {
                    *ratio = new_ratio.clamp(0.1, 0.9);
                    return true;
                }
                first.resize_by_split_id_inner(target, new_ratio)
                    || second.resize_by_split_id_inner(target, new_ratio)
            }
        }
    }

    /// Check whether a pane with the given ID exists in this tree.
    #[inline]
    #[must_use]
    pub fn find_pane(&self, pane_id: PaneId) -> bool {
        match self {
            Self::Leaf(id) => *id == pane_id,
            Self::Split { first, second, .. } => {
                first.find_pane(pane_id) || second.find_pane(pane_id)
            }
        }
    }

    /// Collect all leaf pane IDs in depth-first order.
    #[must_use]
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::with_capacity(self.pane_count());
        self.collect_pane_ids(&mut ids);
        ids
    }

    fn collect_pane_ids(&self, ids: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(id) => ids.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_pane_ids(ids);
                second.collect_pane_ids(ids);
            }
        }
    }

    /// Return the total number of leaf panes in the tree.
    #[inline]
    #[must_use]
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> Rect {
        Rect::new(0.0, 0.0, 1000.0, 800.0)
    }

    #[test]
    fn single_pane_layout() {
        let pane_id = PaneId::new();
        let tree = PaneTree::new(pane_id);
        let layout = tree.layout(viewport());
        assert_eq!(layout.len(), 1);
        assert_eq!(layout[0].0, pane_id);
        assert_eq!(layout[0].1, viewport());
    }

    #[test]
    fn horizontal_split_layout() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let layout = tree.layout(viewport());
        assert_eq!(layout.len(), 2);

        let (id1, r1) = &layout[0];
        let (id2, r2) = &layout[1];
        assert_eq!(*id1, p1);
        assert_eq!(*id2, p2);

        // 1000 * 0.5 - 2.0/2 = 499.0
        assert!((r1.width - 499.0).abs() < f32::EPSILON);
        assert_eq!(r1.x, 0.0);
        // 499.0 + 2.0 = 501.0
        assert!((r2.x - 501.0).abs() < f32::EPSILON);
        assert!((r2.width - 499.0).abs() < f32::EPSILON);
        assert!((r2.x - (r1.x + r1.width) - DIVIDER_WIDTH).abs() < f32::EPSILON);
    }

    #[test]
    fn vertical_split_layout() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Vertical).unwrap();

        let layout = tree.layout(viewport());
        assert_eq!(layout.len(), 2);

        let (_, r1) = &layout[0];
        let (_, r2) = &layout[1];

        // 800 * 0.5 - 2.0/2 = 399.0
        assert!((r1.height - 399.0).abs() < f32::EPSILON);
        // 399.0 + 2.0 = 401.0
        assert!((r2.y - 401.0).abs() < f32::EPSILON);
    }

    #[test]
    fn nested_split_layout() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();
        let p3 = tree.split_pane(p1, SplitDirection::Vertical).unwrap();

        let layout = tree.layout(viewport());
        assert_eq!(layout.len(), 3);

        let find = |id: PaneId| layout.iter().find(|(pid, _)| *pid == id).unwrap().1;
        let r1 = find(p1);
        let r3 = find(p3);
        let r2 = find(p2);

        assert_eq!(r1.x, r3.x);
        assert!(r2.x > r1.x);
        assert!(r3.y > r1.y);
    }

    #[test]
    fn close_pane_promotes_sibling() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        tree.close_pane(p1).unwrap();
        assert!(!tree.find_pane(p1));
        assert!(tree.find_pane(p2));
        assert_eq!(tree.pane_count(), 1);
    }

    #[test]
    fn close_pane_not_found() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let result = tree.close_pane(PaneId::new());
        assert!(result.is_err());
    }

    #[test]
    fn close_last_pane_error() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let result = tree.close_pane(p1);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("cannot close"));
    }

    #[test]
    fn swap_panes_exchanges() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let layout_before = tree.layout(viewport());
        let r1_before = layout_before[0].1;
        let r2_before = layout_before[1].1;

        tree.swap_panes(p1, p2).unwrap();

        let layout_after = tree.layout(viewport());
        let find = |id: PaneId| layout_after.iter().find(|(pid, _)| *pid == id).unwrap().1;
        assert_eq!(find(p2), r1_before);
        assert_eq!(find(p1), r2_before);
    }

    #[test]
    fn swap_panes_same_id_is_noop() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let layout_before = tree.layout(viewport());
        tree.swap_panes(p1, p1).unwrap();
        let layout_after = tree.layout(viewport());

        assert_eq!(layout_before[0].1, layout_after[0].1);
        assert!(tree.find_pane(p1));
        assert!(tree.find_pane(p2));
    }

    #[test]
    fn swap_panes_not_found() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let result = tree.swap_panes(p1, PaneId::new());
        assert!(result.is_err());
    }

    #[test]
    fn resize_split_changes_ratio() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        tree.resize_split(p1, 0.7).unwrap();

        let layout = tree.layout(viewport());
        let r1 = layout[0].1;
        // 1000 * 0.7 - 2.0/2 = 699.0
        assert!((r1.width - 699.0).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_split_nested_targets_immediate_parent() {
        // Tree: Split(Split(Leaf(A), Leaf(B)), Leaf(C))
        // resize_split(A, 0.7) should resize the INNER split (A-B), not the root.
        let a = PaneId::new();
        let mut tree = PaneTree::new(a);
        let c = tree.split_pane(a, SplitDirection::Horizontal).unwrap();
        let b = tree.split_pane(a, SplitDirection::Vertical).unwrap();

        // Before resize: A and B each have 50% of the left column.
        tree.resize_split(a, 0.7).unwrap();

        let layout = tree.layout(viewport());
        let find = |id: PaneId| layout.iter().find(|(pid, _)| *pid == id).unwrap().1;
        let ra = find(a);
        let rb = find(b);
        let rc = find(c);

        // C's rect should be unchanged (root split not affected).
        // A should be taller than B (inner split ratio changed to 0.7).
        assert!(
            ra.height > rb.height,
            "A should be taller than B after resize"
        );
        // C should still occupy the right half.
        assert!(rc.x > ra.x, "C should still be to the right");
    }

    #[test]
    fn pane_ids_returns_all() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();
        let p3 = tree.split_pane(p2, SplitDirection::Vertical).unwrap();

        let ids = tree.pane_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&p1));
        assert!(ids.contains(&p2));
        assert!(ids.contains(&p3));
    }

    #[test]
    fn pane_count_accurate() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        assert_eq!(tree.pane_count(), 1);

        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();
        assert_eq!(tree.pane_count(), 2);

        let p3_target = tree.pane_ids()[1];
        let _p3 = tree
            .split_pane(p3_target, SplitDirection::Vertical)
            .unwrap();
        assert_eq!(tree.pane_count(), 3);
    }

    #[test]
    fn deep_nesting_no_overflow() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let mut current = p1;
        for _ in 0..10 {
            current = tree
                .split_pane(current, SplitDirection::Horizontal)
                .unwrap();
        }
        assert_eq!(tree.pane_count(), 11);

        let layout = tree.layout(viewport());
        assert_eq!(layout.len(), 11);
    }

    #[test]
    fn find_pane_works() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        assert!(tree.find_pane(p1));
        assert!(tree.find_pane(p2));
        assert!(!tree.find_pane(PaneId::new()));
    }

    #[test]
    fn serde_roundtrip() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let json = serde_json::to_string(&tree).unwrap();
        let back: PaneTree = serde_json::from_str(&json).unwrap();

        assert_eq!(tree.pane_count(), back.pane_count());
        for id in tree.pane_ids() {
            assert!(back.find_pane(id));
        }
    }

    #[test]
    fn split_pane_not_found() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let result = tree.split_pane(PaneId::new(), SplitDirection::Horizontal);
        assert!(result.is_err());
    }

    #[test]
    fn resize_split_clamps() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        tree.resize_split(p1, 0.0).unwrap();
        let layout = tree.layout(viewport());
        let r1 = layout[0].1;
        // 1000 * 0.1 - 2.0/2 = 99.0
        assert!((r1.width - 99.0).abs() < f32::EPSILON);

        tree.resize_split(p1, 1.0).unwrap();
        let layout = tree.layout(viewport());
        let r1 = layout[0].1;
        // 1000 * 0.9 - 2.0/2 = 899.0
        assert!((r1.width - 899.0).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_split_root_leaf_error() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let result = tree.resize_split(p1, 0.5);
        assert!(result.is_err());
    }

    // ─── layout_with_dividers tests ───────────────────────────────────────

    #[test]
    fn layout_with_dividers_single_pane_no_dividers() {
        let p1 = PaneId::new();
        let tree = PaneTree::new(p1);
        let (panes, dividers) = tree.layout_with_dividers(viewport());
        assert_eq!(panes.len(), 1);
        assert!(dividers.is_empty());
    }

    #[test]
    fn layout_with_dividers_two_panes_one_divider() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let (panes, dividers) = tree.layout_with_dividers(viewport());
        assert_eq!(panes.len(), 2);
        assert_eq!(dividers.len(), 1);

        let div = &dividers[0];
        assert_eq!(div.direction, SplitDirection::Horizontal);
        // Divider position: at the boundary between p1 and p2
        let find = |id: PaneId| panes.iter().find(|(pid, _)| *pid == id).unwrap().1;
        let r1 = find(p1);
        let r2 = find(p2);
        assert!(div.position > r1.x);
        assert!(div.position < r2.x + r2.width);
    }

    #[test]
    fn layout_with_dividers_nested_produces_two_dividers() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();
        let _p3 = tree.split_pane(p1, SplitDirection::Vertical).unwrap();

        let (panes, dividers) = tree.layout_with_dividers(viewport());
        assert_eq!(panes.len(), 3);
        assert_eq!(dividers.len(), 2);

        // Each divider should have a distinct split_id.
        assert_ne!(dividers[0].split_id, dividers[1].split_id);
    }

    #[test]
    fn layout_with_dividers_matches_layout() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let plain = tree.layout(viewport());
        let (with_div, _) = tree.layout_with_dividers(viewport());

        // Same panes, same rects.
        assert_eq!(plain.len(), with_div.len());
        for (a, b) in plain.iter().zip(with_div.iter()) {
            assert_eq!(a.0, b.0);
            assert_eq!(a.1, b.1);
        }
        let _ = p2;
    }

    // ─── resize_by_split_id tests ─────────────────────────────────────────

    #[test]
    fn resize_by_split_id_changes_ratio() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let (_, dividers) = tree.layout_with_dividers(viewport());
        let split_id = dividers[0].split_id;

        tree.resize_by_split_id(split_id, 0.7).unwrap();

        let layout = tree.layout(viewport());
        let r1 = layout[0].1;
        // 1000 * 0.7 - 2.0/2 = 699.0
        assert!((r1.width - 699.0).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_by_split_id_clamps() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let (_, dividers) = tree.layout_with_dividers(viewport());
        let split_id = dividers[0].split_id;

        // Extreme values should be clamped to 0.1..0.9.
        tree.resize_by_split_id(split_id, 0.0).unwrap();
        let layout = tree.layout(viewport());
        let r1 = layout[0].1;
        assert!((r1.width - 99.0).abs() < f32::EPSILON); // 1000 * 0.1 - 1.0

        tree.resize_by_split_id(split_id, 1.0).unwrap();
        let layout = tree.layout(viewport());
        let r1 = layout[0].1;
        assert!((r1.width - 899.0).abs() < f32::EPSILON); // 1000 * 0.9 - 1.0
    }

    #[test]
    fn resize_by_split_id_not_found() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let _p2 = tree.split_pane(p1, SplitDirection::Horizontal).unwrap();

        let bad_id = SplitId::new();
        let result = tree.resize_by_split_id(bad_id, 0.5);
        assert!(result.is_err());
    }
}
