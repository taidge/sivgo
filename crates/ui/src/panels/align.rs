//! Align & Distribute panel data model (11.4.4).
//!
//! Enums describing alignment, distribution, and "relative to" actions.

/// Alignment operations along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignAction {
    /// Align left edges.
    Left,
    /// Center horizontally.
    Center,
    /// Align right edges.
    Right,
    /// Align top edges.
    Top,
    /// Center vertically.
    Middle,
    /// Align bottom edges.
    Bottom,
}

impl AlignAction {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            AlignAction::Left => "Align Left",
            AlignAction::Center => "Center Horizontal",
            AlignAction::Right => "Align Right",
            AlignAction::Top => "Align Top",
            AlignAction::Middle => "Center Vertical",
            AlignAction::Bottom => "Align Bottom",
        }
    }

    /// All alignment actions in display order.
    pub fn all() -> &'static [AlignAction] {
        &[
            AlignAction::Left,
            AlignAction::Center,
            AlignAction::Right,
            AlignAction::Top,
            AlignAction::Middle,
            AlignAction::Bottom,
        ]
    }
}

/// Distribution operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributeAction {
    /// Make horizontal gaps equal.
    HorizontalGaps,
    /// Make vertical gaps equal.
    VerticalGaps,
    /// Distribute by horizontal centers.
    HorizontalCenters,
    /// Distribute by vertical centers.
    VerticalCenters,
}

impl DistributeAction {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            DistributeAction::HorizontalGaps => "Equal Horizontal Gaps",
            DistributeAction::VerticalGaps => "Equal Vertical Gaps",
            DistributeAction::HorizontalCenters => "Distribute Horizontal Centers",
            DistributeAction::VerticalCenters => "Distribute Vertical Centers",
        }
    }
}

/// What alignment/distribution is relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeTo {
    /// Relative to the bounding box of the current selection.
    Selection,
    /// Relative to the page boundaries.
    Page,
    /// Relative to the drawing (all visible objects).
    Drawing,
    /// Relative to the first selected object.
    FirstSelected,
    /// Relative to the last selected object.
    LastSelected,
}

impl RelativeTo {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            RelativeTo::Selection => "Selection",
            RelativeTo::Page => "Page",
            RelativeTo::Drawing => "Drawing",
            RelativeTo::FirstSelected => "First Selected",
            RelativeTo::LastSelected => "Last Selected",
        }
    }

    /// All options in display order.
    pub fn all() -> &'static [RelativeTo] {
        &[
            RelativeTo::Selection,
            RelativeTo::Page,
            RelativeTo::Drawing,
            RelativeTo::FirstSelected,
            RelativeTo::LastSelected,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_action_labels() {
        assert_eq!(AlignAction::Left.label(), "Align Left");
        assert_eq!(AlignAction::all().len(), 6);
    }

    #[test]
    fn distribute_action_labels() {
        assert_eq!(
            DistributeAction::HorizontalGaps.label(),
            "Equal Horizontal Gaps"
        );
    }

    #[test]
    fn relative_to_all() {
        let all = RelativeTo::all();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], RelativeTo::Selection);
        assert_eq!(all[4], RelativeTo::LastSelected);
    }
}
