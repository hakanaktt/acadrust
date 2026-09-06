//! Example: create viewports in paper space layouts.
//!
//! Demonstrates how to:
//! - Add viewports to the default paper space layout ("Layout1")
//! - Create additional layouts and add viewports to them
//! - Configure viewport scale, view direction, and locking
//!
//! ```
//! cargo run --example viewport_layouts
//! ```

use acadrust::entities::{EntityType, StandardView, Viewport};
use acadrust::types::Vector3;
use acadrust::{CadDocument, DxfVersion, DxfWriter};

fn main() -> acadrust::Result<()> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1027);

    // ── Add some model space geometry ──────────────────────────────────
    let line = acadrust::entities::Line {
        common: Default::default(),
        start: Vector3::new(0.0, 0.0, 0.0),
        end: Vector3::new(100.0, 100.0, 0.0),
        thickness: 0.0,
        normal: Vector3::UNIT_Z,
    };
    doc.add_entity(EntityType::Line(line))?;

    // ── Viewport in the default paper space ("Layout1") ───────────────
    // Create the mandatory overall viewport (ID=1) for Layout1.
    // add_layout() creates this automatically for new layouts,
    // but the default Layout1 needs it explicitly.
    let mut overall_vp = Viewport::new();
    overall_vp.id = 1;
    overall_vp.status = acadrust::entities::ViewportStatusFlags::default_on();
    overall_vp.width = 297.0;
    overall_vp.height = 210.0;
    overall_vp.center = Vector3::new(148.5, 105.0, 0.0);
    doc.add_paper_space_entity(EntityType::Viewport(overall_vp))?;

    let mut vp1 = Viewport::new()
        .with_center(Vector3::new(148.5, 105.0, 0.0))
        .with_view_target(Vector3::new(50.0, 50.0, 0.0))
        .with_scale(1.0)
        .with_locked();
    vp1.id = 2;
    let vp1_handle = doc.add_paper_space_entity(EntityType::Viewport(vp1))?;
    println!("Layout1 viewport handle: {:#X}", vp1_handle.value());

    // ── Create a second layout and add a viewport to it ───────────────
    let _layout2_handle = doc.add_layout("Layout2")?;

    let mut vp2 = Viewport::with_size(Vector3::new(200.0, 150.0, 0.0), 400.0, 300.0);
    vp2.id = 2;
    vp2.set_standard_view(StandardView::NEIsometric);
    vp2.set_scale(0.5);
    vp2.lock();
    let vp2_handle = doc.add_entity_to_layout(EntityType::Viewport(vp2), "Layout2")?;
    println!("Layout2 viewport handle: {:#X}", vp2_handle.value());

    // ── Create a third layout with a front-view viewport ──────────────
    doc.add_layout("Front View")?;

    let mut vp3 = Viewport::new();
    vp3.id = 2;
    vp3.center = Vector3::new(148.5, 105.0, 0.0);
    vp3.width = 297.0;
    vp3.height = 210.0;
    vp3.set_standard_view(StandardView::Front);
    vp3.set_view_height(120.0);
    let vp3_handle = doc.add_entity_to_layout(EntityType::Viewport(vp3), "Front View")?;
    println!("Front View viewport handle: {:#X}", vp3_handle.value());

    // ── Write the DXF file ────────────────────────────────────────────
    let path = "target/viewport_layouts.dxf";
    let writer = DxfWriter::new(&doc);
    writer.write_to_file(path)?;

    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    println!("\nWrote {} ({} bytes)", path, size);
    println!("Open in AutoCAD and check Layout1, Layout2, and Front View tabs.");

    Ok(())
}
