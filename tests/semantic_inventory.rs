use acadrust::{
    CadDocument, EntityType, Line, SemanticEntityV1, SemanticPartV1, SEMANTIC_INVENTORY_VERSION,
};

#[test]
fn public_inventory_visits_each_top_level_payload_once() {
    assert_eq!(SEMANTIC_INVENTORY_VERSION, 1);

    let mut document = CadDocument::new();
    document.add_entity(EntityType::Line(Line::new())).unwrap();

    let expected_tables = document.layers.len()
        + document.line_types.len()
        + document.text_styles.len()
        + document.block_records.len()
        + document.dim_styles.len()
        + document.app_ids.len()
        + document.views.len()
        + document.vports.len()
        + document.ucss.len()
        + document.vx_table.len();
    let expected_objects = document.objects.len();

    let mut headers = 0;
    let mut tables = 0;
    let mut entities = 0;
    let mut objects = 0;
    let mut summaries = 0;
    document.semantic_inventory_v1().visit(|part| match part {
        SemanticPartV1::Header(_) => headers += 1,
        SemanticPartV1::TableRecord(_) => tables += 1,
        SemanticPartV1::Entity(SemanticEntityV1::Typed(EntityType::Line(_))) => entities += 1,
        SemanticPartV1::Entity(_) => entities += 1,
        SemanticPartV1::Object(_) => objects += 1,
        SemanticPartV1::SummaryInfo(_) => summaries += 1,
        _ => {}
    });

    assert_eq!(headers, 1);
    assert_eq!(tables, expected_tables);
    assert_eq!(entities, 1);
    assert_eq!(objects, expected_objects);
    assert_eq!(summaries, 1);
}
