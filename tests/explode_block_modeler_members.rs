use std::f64::consts::FRAC_PI_2;

use acadrust::entities::acis::primitives::build_box;
use acadrust::entities::acis::{SabWriter, SatDocument};
use acadrust::entities::{
    Block, Body, EntityType, Insert, Line, Point, Solid3D, Surface, SurfaceKind,
};
use acadrust::objects::{SolidHistoryBox, SolidHistoryNodeBase, SolidHistoryOperation};
use acadrust::tables::BlockRecord;
use acadrust::types::{Handle, Vector3};
use acadrust::CadDocument;

// EXPLODE takes a block reference apart one level: "Exploding a block
// reference ... replaces it with copies of the objects that compose the
// block" (AutoCAD help, About Exploding Compound Objects). A 3DSOLID, REGION
// or BODY in the block comes back as a copy of itself, placed by the
// reference; it is not broken into its faces or edges.

// A 10 x 10 x 10 box with a corner on the block's base point.
fn box_sat() -> SatDocument {
    build_box([5.0, 5.0, 5.0], 10.0, 10.0, 10.0)
}

// A DWG 2013 or later holds modeler data as binary ACIS (SAB); every solid in
// the OpenCADStudio #2308 drawing is binary.
fn binary_box() -> Solid3D {
    Solid3D::from_sab(SabWriter::write(&box_sat()))
}

// At (100, 0, 0), scale 2, a quarter turn: the box's centre (5, 5, 5) lands
// on (90, 10, 10).
fn reference(block: &str) -> Insert {
    Insert::new(block, Vector3::new(100.0, 0.0, 0.0))
        .with_uniform_scale(2.0)
        .with_rotation(FRAC_PI_2)
}

const PLACED_CENTRE: [f64; 3] = [90.0, 10.0, 10.0];

fn kinds(parts: &[EntityType]) -> Vec<&'static str> {
    parts
        .iter()
        .map(|part| match part {
            EntityType::Solid3D(_) => "3DSOLID",
            EntityType::Region(_) => "REGION",
            EntityType::Body(_) => "BODY",
            EntityType::Surface(_) => "SURFACE",
            EntityType::Line(_) => "LINE",
            EntityType::Point(_) => "POINT",
            EntityType::Insert(_) => "INSERT",
            _ => "other",
        })
        .collect()
}

fn solid(part: &EntityType) -> &Solid3D {
    match part {
        EntityType::Solid3D(solid) => solid,
        _ => panic!("not a 3DSOLID"),
    }
}

fn assert_placed(centre: Option<Vector3>) {
    let centre = centre.expect("the copy's ACIS data should parse");
    let expected = Vector3::new(PLACED_CENTRE[0], PLACED_CENTRE[1], PLACED_CENTRE[2]);
    assert!(
        (centre - expected).length() < 1e-9,
        "centre {centre:?}, expected {expected:?}"
    );
}

#[test]
fn a_binary_solid_in_a_block_explodes_to_a_copy_of_the_solid() {
    let parts = reference("BOX").explode(&[EntityType::Solid3D(binary_box())]);
    assert_eq!(kinds(&parts), ["3DSOLID"]);
    let copy = solid(&parts[0]);
    assert!(copy.acis_data.is_binary);
    assert_placed(copy.acis_data.geometry_centre());
}

#[test]
fn a_sat_solid_in_a_block_is_not_broken_into_regions() {
    let member = Solid3D::from_sat(&box_sat().to_sat_string());
    let parts = reference("BOX").explode(&[EntityType::Solid3D(member)]);
    assert_eq!(kinds(&parts), ["3DSOLID"]);
    assert_placed(solid(&parts[0]).acis_data.geometry_centre());
}

#[test]
fn a_region_in_a_block_stays_a_region() {
    let faces = EntityType::Solid3D(Solid3D::from_sat(&box_sat().to_sat_string())).explode();
    let region = faces.into_iter().next().expect("a face of the box");
    assert_eq!(kinds(std::slice::from_ref(&region)), ["REGION"]);
    let parts = reference("FACE").explode(&[region]);
    assert_eq!(kinds(&parts), ["REGION"]);
}

#[test]
fn a_body_in_a_block_stays_a_body() {
    let member = Body::from_sat(&box_sat().to_sat_string());
    let parts = reference("BOX").explode(&[EntityType::Body(member)]);
    assert_eq!(kinds(&parts), ["BODY"]);
    match &parts[0] {
        EntityType::Body(body) => assert_placed(body.acis_data.geometry_centre()),
        _ => unreachable!(),
    }
}

// The history graph belongs to the member in the block definition; a copy
// that kept its handle would share it.
#[test]
fn an_exploded_solid_does_not_name_its_member_history() {
    let mut member = binary_box();
    member.history_handle = Some(Handle::new(0x4A));
    let parts = reference("BOX").explode(&[EntityType::Solid3D(member)]);
    assert_eq!(kinds(&parts), ["3DSOLID"]);
    assert_eq!(solid(&parts[0]).history_handle, None);
}

#[test]
fn an_exploded_surface_does_not_name_its_member_history() {
    let mut member = Surface::new(SurfaceKind::Extruded);
    member.history_handle = Some(Handle::new(0x4A));
    let parts = reference("SHEET").explode(&[EntityType::Surface(Box::new(member))]);
    assert_eq!(kinds(&parts), ["SURFACE"]);
    match &parts[0] {
        EntityType::Surface(surface) => assert_eq!(surface.history_handle, None),
        _ => unreachable!(),
    }
}

fn add_block(document: &mut CadDocument, name: &str) -> Handle {
    let mut record = BlockRecord::new(name);
    record.handle = document.allocate_handle();
    let owner = record.handle;
    document.block_records.add(record).unwrap();
    let mut marker = Block::new(name, Vector3::ZERO);
    marker.common.owner_handle = owner;
    let marker_handle = document.add_entity(EntityType::Block(marker)).unwrap();
    document
        .block_records
        .get_mut(name)
        .unwrap()
        .block_entity_handle = marker_handle;
    owner
}

fn add_member(document: &mut CadDocument, block: Handle, mut entity: EntityType) -> Handle {
    entity.common_mut().owner_handle = block;
    document.add_entity(entity).unwrap()
}

fn explode_in_model_space(document: &mut CadDocument, mut insert: Insert) -> Vec<EntityType> {
    insert.common.owner_handle = document.header.model_space_block_handle;
    document.explode_entity(&EntityType::Insert(insert))
}

#[test]
fn exploding_a_reference_in_a_document_keeps_its_solid() {
    let mut document = CadDocument::new();
    let block = add_block(&mut document, "BOX");
    let member = add_member(&mut document, block, EntityType::Solid3D(binary_box()));

    let parts = explode_in_model_space(&mut document, reference("BOX"));
    assert_eq!(kinds(&parts), ["3DSOLID"]);
    let copy = solid(&parts[0]);
    assert!(!copy.common.handle.is_null());
    assert_ne!(copy.common.handle, member);
    assert_placed(copy.acis_data.geometry_centre());
}

// OpenCADStudio #2308: each scaffold brace is a block (a 3DSOLID tube, a LINE
// and two node POINTs) nested in a dynamic block. The first EXPLODE gives the
// nested reference; the second has to give the tube back with the line and
// the points, not the line and the points alone.
#[test]
fn exploding_a_nested_block_twice_gives_its_solid_back() {
    let mut document = CadDocument::new();
    let brace = add_block(&mut document, "BRACE");
    add_member(&mut document, brace, EntityType::Solid3D(binary_box()));
    let axis = Line::from_points(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    add_member(&mut document, brace, EntityType::Line(axis));
    let start = Point::at(Vector3::ZERO);
    add_member(&mut document, brace, EntityType::Point(start));
    let end = Point::at(Vector3::new(10.0, 0.0, 0.0));
    add_member(&mut document, brace, EntityType::Point(end));
    let scaffold = add_block(&mut document, "SCAFFOLD");
    let nested = Insert::new("BRACE", Vector3::ZERO);
    add_member(&mut document, scaffold, EntityType::Insert(nested));

    let first = explode_in_model_space(&mut document, reference("SCAFFOLD"));
    assert_eq!(kinds(&first), ["INSERT"]);
    let nested = document.add_entity(first[0].clone()).unwrap();
    let nested = document.get_entity(nested).unwrap().clone();

    let second = document.explode_entity(&nested);
    assert_eq!(kinds(&second), ["3DSOLID", "LINE", "POINT", "POINT"]);
    assert_placed(solid(&second[0]).acis_data.geometry_centre());
}

// Erasing an entity deletes the history graph it names. Doing so to the
// exploded copy must leave the block definition's own solid its history.
#[test]
fn erasing_an_exploded_solid_history_keeps_the_block_definition_history() {
    let mut document = CadDocument::new();
    let block = add_block(&mut document, "BOX");
    let member = add_member(&mut document, block, EntityType::Solid3D(binary_box()));
    let step = SolidHistoryOperation::Box(SolidHistoryBox {
        base: SolidHistoryNodeBase::new(1),
        length: 10.0,
        width: 10.0,
        height: 10.0,
        ..SolidHistoryBox::default()
    });
    let history = document.create_solid_history(member, step).unwrap();

    let parts = explode_in_model_space(&mut document, reference("BOX"));
    assert_eq!(kinds(&parts), ["3DSOLID"]);
    let copy = document.add_entity(parts[0].clone()).unwrap();
    document.delete_solid_history(copy);

    let kept = document
        .solid_history_graph(member)
        .expect("the block member keeps its history");
    assert_eq!(kept.root, history.root);
    assert!(document.objects.contains_key(&history.root));
}
