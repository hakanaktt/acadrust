use acadrust::{DwgReader, DxfReader, DxfWriter};

fn main() {
    let dwg_path = "tests/issue51_rt_issues/civil_example-imperial.dwg";
    let doc = DwgReader::from_file(dwg_path)
        .expect("open dwg")
        .read()
        .expect("read dwg");
    println!("DWG entities: {}", doc.entity_count());

    // Roundtrip 1: DWG -> DXF (file 1)
    DxfWriter::new(&doc)
        .write_to_file("tests/issue51_rt_issues/civil_roundtrip1.dxf")
        .expect("write roundtrip1");
    println!("roundtrip1 written");

    // Roundtrip 2: DXF -> DXF (file 2)
    let doc2 = DxfReader::from_file("tests/issue51_rt_issues/civil_roundtrip1.dxf")
        .expect("open roundtrip1")
        .read()
        .expect("read roundtrip1");
    DxfWriter::new(&doc2)
        .write_to_file("tests/issue51_rt_issues/civil_roundtrip2.dxf")
        .expect("write roundtrip2");
    println!("roundtrip2 written");

    // Sanity: a third cycle must be stable.
    let doc3 = DxfReader::from_file("tests/issue51_rt_issues/civil_roundtrip2.dxf")
        .expect("open roundtrip2")
        .read()
        .expect("read roundtrip2");
    println!("entities roundtrip2: {}", doc2.entity_count());
    println!(
        "objects equal r2 vs r3-read: {}",
        doc2.objects == doc3.objects
    );
}
