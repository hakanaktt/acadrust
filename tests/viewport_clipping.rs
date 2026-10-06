use acadrust::entities::ViewportStatusFlags;

#[test]
fn activation_is_not_boundary_presence() {
    for bits in [0, 0x8000, 0x10000, 0x4000 | 0x8000 | 0x10000] {
        assert_eq!(ViewportStatusFlags::from_bits(bits).to_bits(), bits);
        assert_eq!(
            ViewportStatusFlags::from_bits(bits).non_rectangular_clipping,
            bits & 0x10000 != 0
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn omitted_clipping_flag_defaults_to_disabled() {
    let mut value = serde_json::to_value(ViewportStatusFlags::default_on()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("non_rectangular_clipping");
    let restored: ViewportStatusFlags = serde_json::from_value(value).unwrap();
    assert!(!restored.non_rectangular_clipping);
    assert!(restored.is_on);
}

#[test]
fn literal_dxf_keeps_activation_and_boundary_independently() {
    use acadrust::{DxfReader, EntityType};
    let doc = DxfReader::from_reader(std::io::Cursor::new(include_bytes!(
        "fixtures/viewports/reference-clip-activation.dxf"
    )))
    .unwrap()
    .read()
    .unwrap();
    let view = doc
        .entities()
        .find_map(|e| match e {
            EntityType::Viewport(v) => Some(v),
            _ => None,
        })
        .unwrap();
    assert_ne!(view.status.to_bits() & 0x10000, 0);
    assert_eq!(view.clip_boundary_handle.value(), 0x1002);
}

#[test]
fn dxf_and_dwg_preserve_active_dormant_and_missing_clip() {
    use acadrust::entities::Viewport;
    use acadrust::{
        CadDocument, Circle, DwgReader, DwgWriter, DxfReader, DxfWriter, EntityType, Vector3,
    };
    use std::io::Cursor;
    for enabled in [false, true] {
        for has_boundary in [false, true] {
            let mut doc = CadDocument::new();
            let circle = if has_boundary {
                doc.add_entity_to_layout(
                    EntityType::Circle(Circle::from_center_radius(
                        Vector3::new(100., 75., 0.),
                        50.,
                    )),
                    "Layout1",
                )
                .unwrap()
            } else {
                acadrust::Handle::NULL
            };
            let mut view = Viewport::new();
            view.id = 2;
            view.center = Vector3::new(100., 75., 0.);
            view.width = 160.;
            view.height = 100.;
            view.status =
                ViewportStatusFlags::from_bits(0x8000 | if enabled { 0x10000 } else { 0 });
            view.clip_boundary_handle = circle;
            doc.add_entity_to_layout(EntityType::Viewport(view), "Layout1")
                .unwrap();
            for dwg in [false, true] {
                let restored = if dwg {
                    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
                        .read()
                        .unwrap()
                } else {
                    DxfReader::from_reader(Cursor::new(
                        DxfWriter::new(&doc).write_to_vec().unwrap(),
                    ))
                    .unwrap()
                    .read()
                    .unwrap()
                };
                let view = restored
                    .entities()
                    .find_map(|entity| match entity {
                        EntityType::Viewport(v) if v.width == 160. => Some(v),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(view.status.to_bits() & 0x10000 != 0, enabled, "dwg={dwg}");
                assert_eq!(
                    !view.clip_boundary_handle.is_null(),
                    has_boundary,
                    "dwg={dwg}"
                );
                if has_boundary {
                    assert!(matches!(
                        restored.get_entity(view.clip_boundary_handle),
                        Some(EntityType::Circle(_))
                    ));
                }
            }
        }
    }
}
