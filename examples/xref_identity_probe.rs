//! Bounded diagnostic cases for independent FileDepList reader qualification.
use acadrust::{document::FileDependency, types::DxfVersion, CadDocument, DwgWriter};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&dir)?;
    for (name, version) in [
        ("ac18", DxfVersion::AC1018),
        ("ac21", DxfVersion::AC1021),
        ("ac32", DxfVersion::AC1032),
    ] {
        for full in [false, true] {
            let mut doc = CadDocument::with_version(version);
            doc.file_dependencies.push(FileDependency {
                feature: "Acad:XRef".into(),
                file_name: "source.dwg".into(),
                found_path: if full {
                    "./source.dwg".into()
                } else {
                    String::new()
                },
                fingerprint_guid: Some("{E5B4A9AF-CF20-4FC9-A30F-FFB20FD987E1}".into()),
                version_guid: full.then(|| "{11111111-2222-3333-4444-555555555555}".into()),
                timestamp: 123,
                file_size: 456,
                affects_graphics: true,
                reference_count: 1,
            });
            std::fs::write(
                dir.join(format!("{name}-{full}.dwg")),
                DwgWriter::write_to_vec(&doc)?,
            )?;
        }
    }
    Ok(())
}
