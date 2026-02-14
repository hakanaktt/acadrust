/// Cross-library line test: write simple DWGs, then cross-read the other library's output.
///
/// Usage:
///   cargo run --bin cross_line_test -- write-lines <output_dir>
///   cargo run --bin cross_line_test -- cross-read  <dwg_dir>
///   cargo run --bin cross_line_test -- self-read   <dwg_dir>  (read our own output back)

use std::fs;
use std::path::{Path, PathBuf};

use acadrust::entities::*;
use acadrust::io::dwg::reader::dwg_reader::{DwgReader, DwgReaderConfiguration};
use acadrust::io::dwg::writer::dwg_writer::DwgWriter;
use acadrust::types::DxfVersion;
use acadrust::CadDocument;

const VERSIONS: &[(DxfVersion, &str)] = &[
    (DxfVersion::AC1012, "AC1012"),
    (DxfVersion::AC1014, "AC1014"),
    (DxfVersion::AC1015, "AC1015"),
    (DxfVersion::AC1018, "AC1018"),
    (DxfVersion::AC1021, "AC1021"),
    (DxfVersion::AC1024, "AC1024"),
    (DxfVersion::AC1027, "AC1027"),
    (DxfVersion::AC1032, "AC1032"),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage:");
        eprintln!("  cross_line_test write-lines <output_dir>");
        eprintln!("  cross_line_test cross-read  <dwg_dir>");
        eprintln!("  cross_line_test self-read   <dwg_dir>");
        std::process::exit(2);
    }

    let cmd = args[1].as_str();
    let dir = PathBuf::from(&args[2]);

    match cmd {
        "write-lines" => write_lines(&dir),
        "cross-read" => cross_read(&dir),
        "self-read" => cross_read(&dir),
        _ => {
            eprintln!("Unknown command: {cmd}");
            std::process::exit(2);
        }
    }
}

fn write_lines(output_dir: &Path) {
    fs::create_dir_all(output_dir).expect("create output dir");

    let mut ok = 0;
    let mut fail = 0;

    for &(version, code) in VERSIONS {
        let out_path = output_dir.join(format!("line_{code}.dwg"));
        match write_line_dwg(version, &out_path) {
            Ok(size) => {
                println!("OK  line_{code}.dwg ({size} bytes)");
                ok += 1;
            }
            Err(e) => {
                println!("ERR line_{code}.dwg :: {e}");
                fail += 1;
            }
        }
    }

    println!("write-lines done. ok={ok} fail={fail}");
    if fail > 0 {
        std::process::exit(1);
    }
}

fn write_line_dwg(version: DxfVersion, path: &Path) -> Result<u64, String> {
    let mut doc = CadDocument::new();
    doc.version = version;

    let line = Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0);
    doc.add_entity(EntityType::Line(line))
        .map_err(|e| e.to_string())?;

    let bytes = DwgWriter::write(&doc).map_err(|e| e.to_string())?;
    fs::write(path, &bytes).map_err(|e| e.to_string())?;
    Ok(bytes.len() as u64)
}

fn cross_read(dwg_dir: &Path) {
    if !dwg_dir.exists() {
        eprintln!("Directory not found: {}", dwg_dir.display());
        std::process::exit(2);
    }

    let mut files: Vec<PathBuf> = fs::read_dir(dwg_dir)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("dwg"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();

    println!("Cross-reading {} DWG files from {}", files.len(), dwg_dir.display());
    println!("---");

    let mut ok = 0;
    let mut fail = 0;

    for file in files {
        let name = file.file_name().unwrap().to_string_lossy().to_string();

        // Try strict first
        let strict_result = read_doc(&file, false);
        let failsafe_result = if strict_result.is_err() {
            Some(read_doc(&file, true))
        } else {
            None
        };

        let doc = match (&strict_result, &failsafe_result) {
            (Ok(d), _) => {
                println!("OK  {name}  (strict)");
                Some(d)
            }
            (Err(_), Some(Ok(d))) => {
                println!("OK  {name}  (failsafe only; strict failed)");
                Some(d)
            }
            (Err(e1), Some(Err(e2))) => {
                println!("ERR {name}  strict: {e1}");
                println!("              failsafe: {e2}");
                fail += 1;
                println!("---");
                continue;
            }
            (Err(e), None) => {
                println!("ERR {name}  :: {e}");
                fail += 1;
                println!("---");
                continue;
            }
        };

        if let Some(doc) = doc {
            ok += 1;
            println!("    version={:?}", doc.version);
            println!("    entities={}", doc.entity_count());
            println!("    layers={}", doc.layers.iter().count());
            println!("    linetypes={}", doc.line_types.iter().count());
            println!("    textstyles={}", doc.text_styles.iter().count());
            println!("    dimstyles={}", doc.dim_styles.iter().count());
            println!("    blocks={}", doc.block_records.iter().count());

            // Entity type histogram
            let mut hist: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for entity in doc.entities() {
                let key = entity.as_entity().entity_type().to_string();
                *hist.entry(key).or_insert(0) += 1;
            }
            for (k, v) in &hist {
                println!("    entity:{k}={v}");
            }

            // Check for Line entity
            for entity in doc.entities() {
                if let EntityType::Line(l) = entity {
                    println!(
                        "    line.start=({},{},{})",
                        l.start.x, l.start.y, l.start.z
                    );
                    println!(
                        "    line.end=({},{},{})",
                        l.end.x, l.end.y, l.end.z
                    );
                    break;
                }
            }
        }

        println!("---");
    }

    println!("cross-read done. ok={ok} fail={fail}");
    if fail > 0 {
        std::process::exit(1);
    }
}

fn read_doc(path: &Path, failsafe: bool) -> Result<CadDocument, String> {
    let reader = DwgReader::from_file(path).map_err(|e| format!("{e}"))?;
    let config = DwgReaderConfiguration {
        failsafe,
        keep_unknown_entities: true,
    };
    reader.with_config(config).read().map_err(|e| format!("{e}"))
}
