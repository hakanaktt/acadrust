use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion};
use acadrust::{CadDocument, DxfWriter};

fn layer_table(text: &str) -> &str {
    let start = text.find("TABLE\r\n  2\r\nLAYER\r\n").expect("LAYER table");
    let end = start + text[start..].find("ENDTAB").expect("LAYER ENDTAB");
    &text[start..end]
}

fn layer_handles(table: &str) -> Vec<u64> {
    table
        .split("  0\r\nLAYER\r\n")
        .skip(1)
        .filter_map(|record| {
            let marker = "  5\r\n";
            let start = record.find(marker)? + marker.len();
            let end = record[start..].find("\r\n")? + start;
            u64::from_str_radix(&record[start..end], 16).ok()
        })
        .collect()
}

#[test]
fn ac1032_layer_output_has_valid_header_handles_and_plotstyle() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for (name, aci) in [("LINE", 5), ("POLYLINES", 1), ("pink", 6)] {
        let mut layer = Layer::new(name);
        layer.color = Color::from_index(aci);
        doc.layers.add(layer).unwrap();
    }

    let output = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let maintenance = output
        .split("$ACADMAINTVER\r\n")
        .nth(1)
        .expect("$ACADMAINTVER")
        .split("\r\n")
        .collect::<Vec<_>>();
    assert_eq!(maintenance[0], " 90");
    assert_eq!(maintenance[1], "     0");

    let table = layer_table(&output);
    let handles = layer_handles(table);
    assert_eq!(handles.len(), 4);
    assert!(handles.iter().all(|handle| *handle != 0));
    let unique = handles.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), handles.len());

    let handseed = output
        .split("$HANDSEED\r\n  5\r\n")
        .nth(1)
        .and_then(|rest| rest.split("\r\n").next())
        .and_then(|value| u64::from_str_radix(value, 16).ok())
        .expect("$HANDSEED");
    assert!(handles.iter().all(|handle| *handle < handseed));

    let user_layers = table.split("  0\r\nLAYER\r\n").skip(2);
    assert!(user_layers
        .clone()
        .all(|record| record.contains("\r\n390\r\n")));
}
