use acadrust::entities::hatch::BoundaryEdge;
use acadrust::EntityType;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("Usage: diag_hatch <file.dwg>");
    let doc = acadrust::DwgReader::from_file(std::path::Path::new(&path))
        .expect("open")
        .read()
        .expect("read");

    let mut total = 0;
    let mut zero_paths = 0;
    let mut valid = 0;

    for e in doc.entities() {
        if let EntityType::Hatch(h) = e {
            total += 1;
            let grad_colors = h.gradient_color.colors.len();
            print!(
                "HATCH {:?} is_solid={} paths={} pattern={:?} grad_colors={}",
                h.common.handle,
                h.is_solid,
                h.paths.len(),
                h.pattern.name,
                grad_colors
            );
            if h.paths.is_empty() {
                zero_paths += 1;
                print!(" <NO BOUNDARY>");
            } else {
                valid += 1;
            }
            println!();
            for (i, path) in h.paths.iter().enumerate() {
                if path.edges.is_empty() {
                    continue;
                }
                println!("  path[{}] edges={}", i, path.edges.len());
                for (j, edge) in path.edges.iter().enumerate() {
                    match edge {
                        BoundaryEdge::Line(l) => println!(
                            "    [{}] Line ({:.1},{:.1})->({:.1},{:.1})",
                            j, l.start.x, l.start.y, l.end.x, l.end.y
                        ),
                        BoundaryEdge::Polyline(p) => println!(
                            "    [{}] Polyline verts={} closed={}",
                            j,
                            p.vertices.len(),
                            p.is_closed
                        ),
                        BoundaryEdge::CircularArc(a) => {
                            println!("    [{}] Arc r={:.3}", j, a.radius)
                        }
                        _ => println!("    [{}] other", j),
                    }
                }
            }
        }
    }
    eprintln!(
        "Totals: hatches={}  zero_paths={}  valid_with_edges={}",
        total, zero_paths, valid
    );
}
