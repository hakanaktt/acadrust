use acadrust::types::Transparency;

#[test]
fn dwg_explicit_transparency_uses_the_by_alpha_method() {
    assert_eq!(Transparency::ByLayer.to_alpha_value(), 0x0000_0000);
    assert_eq!(Transparency::ByBlock.to_alpha_value(), 0x0100_0000);
    assert_eq!(Transparency::Explicit(32).to_alpha_value(), 0x0200_00DF);
    assert_eq!(Transparency::OPAQUE.to_alpha_value(), 0x0200_00FF);
}

#[test]
fn legacy_method_three_remains_readable() {
    assert_eq!(
        Transparency::from_alpha_value(0x0300_00DF),
        Transparency::Explicit(32)
    );
}
