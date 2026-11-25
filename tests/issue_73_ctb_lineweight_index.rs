//! A style's lineweight number picks the lineweight list's entry one before it (issue #73).
//!
//! The plot style editor's lineweight choices are "Use object lineweight" followed by the
//! table's list, so a stored number N means entry N - 1 of the list and 0 means the object's own.
//! Every table here is a small synthetic one, written out with `to_bytes` and read back with
//! `from_bytes`, so the numbers go through the same text the parser reads from a file.

use acadrust::plot_style::PlotStyleTable;

/// A table with the four listed widths 0.0, 0.5, 1.0 and 2.0 mm, and one style per case:
/// ACI 1 to 4 hold the numbers 1 to 4, ACI 5 holds one past the list, ACI 6 holds 0 and
/// ACI 7 holds 255.
fn table(apply_factor: bool, scale_factor: f32) -> PlotStyleTable {
    let mut table = PlotStyleTable::identity("synthetic.ctb");
    table.lineweights = vec![0.0, 0.5, 1.0, 2.0];
    table.apply_factor = apply_factor;
    table.scale_factor = scale_factor;
    for (aci, number) in [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 0), (7, 255)] {
        table.aci_entries[aci].lineweight = number;
    }
    let bytes = table.to_bytes().expect("write");
    PlotStyleTable::from_bytes("synthetic.ctb", &bytes).expect("read")
}

#[test]
fn style_1_picks_the_first_listed_width() {
    assert_eq!(table(false, 1.0).resolve_lineweight(1), Some(0.0));
}

#[test]
fn style_2_picks_the_second_listed_width() {
    assert_eq!(table(false, 1.0).resolve_lineweight(2), Some(0.5));
}

#[test]
fn the_last_listed_width_is_reachable() {
    let table = table(false, 1.0);
    assert_eq!(table.resolve_lineweight(3), Some(1.0));
    assert_eq!(table.resolve_lineweight(4), Some(2.0));
}

#[test]
fn a_number_past_the_list_keeps_the_object_lineweight() {
    assert_eq!(table(false, 1.0).resolve_lineweight(5), None);
}

#[test]
fn style_0_keeps_the_object_lineweight() {
    assert_eq!(table(false, 1.0).resolve_lineweight(6), None);
}

#[test]
fn style_255_keeps_the_object_lineweight() {
    assert_eq!(table(false, 1.0).resolve_lineweight(7), None);
}

#[test]
fn the_scale_factor_applies_to_the_entry_that_was_picked() {
    let table = table(true, 0.5);
    assert_eq!(table.resolve_lineweight(2), Some(0.25));
    assert_eq!(table.resolve_lineweight(4), Some(1.0));
}

#[test]
fn an_unlisted_aci_has_no_override() {
    assert_eq!(table(false, 1.0).resolve_lineweight(8), None);
}
