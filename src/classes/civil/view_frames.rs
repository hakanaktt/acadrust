//! View frames and match lines of a plan production run.

/// One view frame of a view frame group, with the match lines on its
/// sides. The frames of a run hang in a chain: the match line after one
/// frame is the one before the next.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewFrame {
    /// The frame's name (`VF - 1`, ...).
    pub name: String,
    /// The view frame group (`AECC_VIEWFRAME_GROUP`) the frame belongs to.
    pub group: u64,
    /// The match line on the frame's station-low side, 0 at the first.
    pub match_line_before: u64,
    /// The match line on the frame's station-high side, 0 at the last.
    pub match_line_after: u64,
}

/// One match line between two view frames.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchLine {
    /// The match line's name (`ML - 1`, ...).
    pub name: String,
    /// The view frame group the match line belongs to.
    pub group: u64,
    /// The frame on the line's station-low side.
    pub view_frame_before: u64,
    /// The frame on the line's station-high side.
    pub view_frame_after: u64,
}

/// A view frame record: its name in the text stream, then its group and
/// side match lines among its references:
///
/// ```text
/// references: owner, owner, style, style, group, match line before
///             (0 at the first frame), match line after (0 at the last)
/// ```
pub(super) fn view_frame(strings: &[String], refs: &[(u64, u8)]) -> Option<ViewFrame> {
    let (group, before, after) = match refs {
        [(.., 4), (.., 3), (.., 5), (.., 5), (group, 5), (before, 4), (after, 4)] => (group, before, after),
        _ => return None,
    };
    Some(ViewFrame {
        name: strings.get(2)?.clone(),
        group: *group,
        match_line_before: *before,
        match_line_after: *after,
    })
}

/// A match line record:
///
/// ```text
/// references: view frame before, group, view frame after, owner, owner,
///             style
/// ```
pub(super) fn match_line(strings: &[String], refs: &[(u64, u8)]) -> Option<MatchLine> {
    let (before, group, after) = match refs {
        [(before, 4), (group, 5), (after, 4), (.., 4), (.., 3), (.., 5)] => (before, group, after),
        _ => return None,
    };
    Some(MatchLine {
        name: strings.get(2)?.clone(),
        group: *group,
        view_frame_before: *before,
        view_frame_after: *after,
    })
}
