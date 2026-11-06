//! Units of MTEXT paragraph spacing (`\p…b…;`, `\p…a…;`) and exact line
//! spacing (`\p…se…;`), issue #306.
//!
//! Measured on synthetic MTEXT at text heights 2 and 10, in a metric
//! (INSUNITS 4) and an imperial (INSUNITS 1) drawing. All gave the same
//! result at scale. None of the values is in inches or in drawing units:
//!
//! - `b#` / `a#` leave `#` × text height before / after the paragraph.
//! - `sm#` is a factor of the default line pitch, 5/3 × text height.
//! - `se#` is the same factor of the pitch, but it also moves the baseline
//!   to `#` × text height below the top of the line's slot (1 × text height
//!   otherwise).

use acadrust::entities::mtext_format::{parse_mtext, MTextLineSpacing};

const HEIGHTS: [f64; 2] = [2.0, 10.0];

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-12
}

#[test]
fn spacing_before_and_after_are_multiples_of_the_text_height() {
    let doc = parse_mtext(r"{\pxb2,a0.5;First\PSecond}", false);
    let properties = &doc.paragraphs[0].properties;
    for h in HEIGHTS {
        let before = properties.spacing_before_distance(h).unwrap();
        let after = properties.spacing_after_distance(h).unwrap();
        assert!(close(before, 2.0 * h), "b2 at H={h}: {before}");
        assert!(close(after, 0.5 * h), "a0.5 at H={h}: {after}");
    }
}

#[test]
fn absent_spacing_has_no_distance() {
    let doc = parse_mtext("Plain", false);
    let properties = &doc.paragraphs[0].properties;
    assert_eq!(properties.spacing_before_distance(2.0), None);
    assert_eq!(properties.spacing_after_distance(2.0), None);
}

#[test]
fn multiple_line_spacing_is_a_factor_of_five_thirds_the_text_height() {
    let doc = parse_mtext(r"{\pxsm1.5;One\POne more}", false);
    let spacing = doc.paragraphs[0].properties.line_spacing.unwrap();
    assert_eq!(spacing, MTextLineSpacing::Multiple(1.5));
    for h in HEIGHTS {
        let pitch = spacing.line_pitch(h, 1.0);
        assert!(close(pitch, 1.5 * 5.0 / 3.0 * h), "sm1.5 at H={h}: {pitch}");
        assert!(close(spacing.baseline_offset(h), h));
    }
}

#[test]
fn exact_line_spacing_is_a_factor_not_a_distance() {
    for (code, factor) in [(r"{\pxse2;Two}", 2.0), (r"{\pxse0.5;Half}", 0.5)] {
        let doc = parse_mtext(code, false);
        let spacing = doc.paragraphs[0].properties.line_spacing.unwrap();
        assert_eq!(spacing, MTextLineSpacing::Exact(factor));
        for h in HEIGHTS {
            let pitch = spacing.line_pitch(h, 1.0);
            assert!(
                close(pitch, factor * 5.0 / 3.0 * h),
                "{code} pitch at H={h}: {pitch}"
            );
        }
    }
}

#[test]
fn exact_line_spacing_moves_the_baseline_within_the_line_slot() {
    for h in HEIGHTS {
        // se2: the first line drops (2 - 1) × H below where sm2 puts it.
        let exact = MTextLineSpacing::Exact(2.0);
        let multiple = MTextLineSpacing::Multiple(2.0);
        assert!(close(exact.baseline_offset(h), 2.0 * h));
        assert!(close(exact.baseline_offset(h) - multiple.baseline_offset(h), h));
        // se0.5 raises it by half a height.
        let half = MTextLineSpacing::Exact(0.5);
        assert!(close(half.baseline_offset(h) - h, -0.5 * h));
    }
}

#[test]
fn default_line_spacing_uses_the_entity_factor() {
    for h in HEIGHTS {
        let pitch = MTextLineSpacing::Default.line_pitch(h, 1.25);
        assert!(close(pitch, 1.25 * 5.0 / 3.0 * h), "default at H={h}: {pitch}");
        assert!(close(MTextLineSpacing::Default.baseline_offset(h), h));
    }
}
