#[test]
fn unknown_leader_source_enums_cannot_silently_become_defaults() {
    use acadrust::entities::{EntityType, Leader, MultiLeader};
    use acadrust::{CadDocument, DxfReader, DxfWriter, Vector3};
    for (entity_name, code, entity) in [
        (
            "LEADER",
            72,
            EntityType::Leader(Leader::two_point(Vector3::ZERO, Vector3::UNIT_X)),
        ),
        (
            "MULTILEADER",
            170,
            EntityType::MultiLeader(Box::new(MultiLeader::with_text(
                "closed codes",
                Vector3::UNIT_X,
                vec![Vector3::ZERO, Vector3::UNIT_X],
            ))),
        ),
    ] {
        let mut document = CadDocument::new();
        document.add_entity(entity).unwrap();
        let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let lines: Vec<_> = text.lines().collect();
        let mut pairs: Vec<_> = lines
            .chunks_exact(2)
            .map(|p| (p[0].trim().parse::<i32>().unwrap(), p[1].to_string()))
            .collect();
        let start = pairs
            .iter()
            .position(|(c, v)| *c == 0 && v.trim() == entity_name)
            .unwrap();
        let end = start
            + 1
            + pairs[start + 1..]
                .iter()
                .position(|(c, _)| *c == 0)
                .unwrap();
        let position = (start..end).rev().find(|i| pairs[*i].0 == code).unwrap();
        pairs[position].1 = "99".into();
        let changed = pairs
            .into_iter()
            .map(|(c, v)| format!("{c}\r\n{v}\r\n"))
            .collect::<String>();
        let outcome = DxfReader::from_reader(std::io::Cursor::new(changed.into_bytes()))
            .unwrap()
            .read_with_stats();
        assert!(
            outcome.is_err(),
            "{entity_name} unknown code99 was normalized instead of diagnosed"
        );
    }
}
