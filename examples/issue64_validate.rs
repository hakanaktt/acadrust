use acadrust::{DxfReader, DxfWriter};

fn main() {
    let input = "tests/issue64/byblock_repro_input.dxf";
    let output = "tests/issue64/byblock_repro_output.dxf";

    let document = DxfReader::from_file(input)
        .expect("open")
        .read()
        .expect("read");
    DxfWriter::new(&document)
        .write_to_file(output)
        .expect("write");
    println!("round-trip written");

    let ts = document.text_styles.get("Standard").unwrap().handle;
    let byblock = document.line_types.get("ByBlock").unwrap().handle;
    let dimtxsty = document.dim_styles.get("Standard").unwrap().dimtxsty_handle;
    println!("text style Standard = {ts:?}");
    println!("ByBlock LTYPE       = {byblock:?}");
    println!("DIMSTYLE dimtxsty   = {dimtxsty:?}");
    assert_ne!(
        dimtxsty, byblock,
        "DIMSTYLE text style points at the ByBlock linetype"
    );

    let out = std::fs::read_to_string(output).unwrap();
    let lines: Vec<&str> = out.split("\r\n").collect();
    for i in 0..lines.len() - 3 {
        if lines[i] == "DIMSTYLE" && lines[i - 1] == "  0" {
            let mut j = i;
            let mut h = String::new();
            let mut ts340 = String::new();
            while j < lines.len() - 1 {
                if lines[j] == "  5" && h.is_empty() {
                    h = lines[j + 1].trim().to_string();
                }
                if lines[j].trim() == "340" {
                    ts340 = lines[j + 1].trim().to_string();
                }
                if lines[j] == "  0" && j > i {
                    break;
                }
                j += 1;
            }
            println!(
                "output DIMSTYLE handle={h} 340-> {ts340} (ByBlock LTYPE = {:X})",
                byblock.value()
            );
            assert_ne!(
                ts340,
                format!("{:X}", byblock.value()),
                "340 points at the ByBlock linetype"
            );
            break;
        }
    }
    println!("ISSUE #64 VALIDATION PASSED on the reporter's sample");
}
