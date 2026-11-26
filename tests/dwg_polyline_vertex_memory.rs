//! Reading the VERTEX records of 3D polylines from DWG costs memory in
//! proportion to the vertex data, not a full entity record per vertex. The
//! vertices are held until every record is decoded and the polylines are
//! assembled, so a per-vertex entity record sets the read's peak on drawings
//! with many 3D polyline vertices.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Cursor;
use std::mem::size_of;
use std::sync::atomic::{AtomicIsize, Ordering};

use acadrust::entities::{EntityCommon, EntityType, Polyline2D, Polyline3D, Vertex2D};
use acadrust::types::Vector3;
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// Counts the bytes currently allocated by the whole process, and the most
/// that were allocated at once. This file holds one test, so no other test
/// allocates while it measures.
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);

fn add(bytes: isize) {
    let live = LIVE.fetch_add(bytes, Ordering::SeqCst) + bytes;
    PEAK.fetch_max(live, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            add(layout.size() as isize);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        add(-(layout.size() as isize));
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            add(new_size as isize - layout.size() as isize);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const POLYLINES: usize = 1_000;
const VERTICES: usize = 50;

/// Peak bytes allocated while reading a drawing of `POLYLINES` polylines with
/// `VERTICES` vertices each, above what was allocated before the read.
fn read_peak(polyline: impl Fn(Vec<Vector3>) -> EntityType) -> isize {
    let mut doc = CadDocument::default();
    for p in 0..POLYLINES {
        let points = (0..VERTICES)
            .map(|v| Vector3::new(p as f64, v as f64, (p + v) as f64))
            .collect();
        doc.add_entity(polyline(points)).expect("polyline");
    }
    let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
    drop(doc);

    let before = LIVE.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let read = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg");
    let peak = PEAK.load(Ordering::SeqCst) - before;
    let read_vertices: usize = read
        .entities()
        .map(|e| match e {
            EntityType::Polyline2D(p) => p.vertices.len(),
            EntityType::Polyline3D(p) => p.vertices.len(),
            _ => 0,
        })
        .sum();
    assert_eq!(read_vertices, POLYLINES * VERTICES);
    peak
}

#[test]
fn a_3d_polyline_vertex_costs_the_dwg_read_peak_no_more_than_a_2d_one() {
    // A pending 2D vertex holds only its vertex data, which is larger than a
    // 3D vertex's. A 3D vertex may add its layer, but not an entity record.
    let peak_2d = read_peak(|points| {
        let mut polyline = Polyline2D::new();
        for point in points {
            polyline.add_vertex(Vertex2D::new(point));
        }
        EntityType::Polyline2D(polyline)
    });
    let peak_3d = read_peak(|points| EntityType::Polyline3D(Polyline3D::from_points(points)));
    let extra_per_vertex = (peak_3d - peak_2d) / (POLYLINES * VERTICES) as isize;
    let entity_record = size_of::<EntityCommon>() as isize;
    assert!(
        extra_per_vertex < entity_record / 4,
        "a 3D polyline vertex adds {extra_per_vertex} bytes to the read's peak over a 2D one \
         (an entity record is {entity_record} bytes)"
    );
}
