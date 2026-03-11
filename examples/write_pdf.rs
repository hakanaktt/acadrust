//! Write a simple CAD drawing to PDF.
//!
//! Creates a small drawing with various entity types and exports it to
//! `drawing.pdf` in the current directory.
//!
//! Run with:
//! ```sh
//! cargo run --example write_pdf
//! ```

use acadrust::{
    CadDocument, EntityType, PdfWriter,
    entities::{Arc, Circle, Line, LwPolyline, MText, Point, Text},
    io::pdf::PdfPageSize,
    types::{Color, Vector2, Vector3},
};

fn main() -> acadrust::error::Result<()> {
    let mut doc = CadDocument::new();

    // Line
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 100.0, 0.0, 0.0,
    )))?;

    // Circle
    let mut circle = Circle::from_coords(50.0, 50.0, 0.0, 30.0);
    circle.common.color = Color::CYAN;
    doc.add_entity(EntityType::Circle(circle))?;

    // Arc (quarter circle)
    let mut arc = Arc::from_coords(
        0.0,
        0.0,
        0.0,
        20.0,
        0.0,
        std::f64::consts::PI / 2.0,
    );
    arc.common.color = Color::RED;
    doc.add_entity(EntityType::Arc(arc))?;

    // Lightweight polyline (triangle)
    let mut poly = LwPolyline::from_points(vec![
        Vector2::new(10.0, 10.0),
        Vector2::new(40.0, 10.0),
        Vector2::new(25.0, 40.0),
    ]);
    poly.is_closed = true;
    poly.common.color = Color::GREEN;
    doc.add_entity(EntityType::LwPolyline(poly))?;

    // Point
    let mut pt = Point::from_coords(80.0, 80.0, 0.0);
    pt.common.color = Color::BLUE;
    doc.add_entity(EntityType::Point(pt))?;

    // Text label
    let mut text = Text::with_value("acadrust", Vector3::new(5.0, 90.0, 0.0));
    text.height = 6.0;
    text.common.color = Color::MAGENTA;
    doc.add_entity(EntityType::Text(text))?;

    // Multi-line text
    let mut mtext = MText::with_value("Hello\\PPDF!", Vector3::new(60.0, 5.0, 0.0));
    mtext.height = 5.0;
    doc.add_entity(EntityType::MText(mtext))?;

    // Export to PDF (A4 page)
    let path = "drawing.pdf";
    PdfWriter::new(&doc)
        .with_page_size(PdfPageSize::A4)
        .write_to_file(path)?;
    println!("Wrote {path}");

    Ok(())
}
