use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::rect::{Rect, DIVIDER_WIDTH};
use crate::surface::SplitDirection;
use crate::types::PaneId;

/// Binary tree representing pane layout within a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneTree {
    /// A leaf node containing a single pane.
    Leaf(PaneId),
    /// A split node dividing space between two children.
    Split {
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

        assert!((r1.width - 498.0).abs() < f32::EPSILON);
        assert_eq!(r1.x, 0.0);
        assert!((r2.x - 502.0).abs() < f32::EPSILON);
        assert!((r2.width - 498.0).abs() < f32::EPSILON);
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

        assert!((r1.height - 398.0).abs() < f32::EPSILON);
        assert!((r2.y - 402.0).abs() < f32::EPSILON);
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
        assert!((r1.width - 698.0).abs() < f32::EPSILON);
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
        assert!((r1.width - 98.0).abs() < f32::EPSILON);

        tree.resize_split(p1, 1.0).unwrap();
        let layout = tree.layout(viewport());
        let r1 = layout[0].1;
        assert!((r1.width - 898.0).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_split_root_leaf_error() {
        let p1 = PaneId::new();
        let mut tree = PaneTree::new(p1);
        let result = tree.resize_split(p1, 0.5);
        assert!(result.is_err());
    }
}
