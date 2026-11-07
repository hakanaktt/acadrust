//! The decompressed objects section is freed once every record is decoded,
//! so it does not stay allocated next to the finished entities while the
//! post-pass (polyline assembly, block membership, XDATA) runs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Cursor;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};

use acadrust::entities::{EntityType, MText};
use acadrust::types::Vector3;
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// Counts the bytes currently allocated by the whole process. This file holds
/// one test, so no other test allocates while it measures.
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(layout.size() as isize, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size() as isize, Ordering::SeqCst);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            LIVE.fetch_add(new_size as isize - layout.size() as isize, Ordering::SeqCst);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const TEXTS: usize = 400;
const TEXT_LEN: usize = 4000;

#[test]
fn objects_section_is_freed_before_the_post_pass() {
    let mut doc = CadDocument::default();
    for i in 0..TEXTS {
        let value: String = (0..TEXT_LEN)
            .map(|j| char::from(b'a' + ((i * 7 + j * 13) % 26) as u8))
            .collect();
        doc.add_entity(EntityType::MText(Box::new(MText::with_value(
            value,
            Vector3::new(i as f64, 0.0, 0.0),
        ))))
        .expect("mtext");
    }
    let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
    drop(doc);

    // Live bytes at the last progress report before the post-pass (pass 2
    // ends at 870) and at the first report of the post-pass (875).
    let marks: Arc<Mutex<(Option<isize>, Option<isize>)>> = Arc::default();
    let progress_marks = Arc::clone(&marks);
    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    reader.set_progress_callback(Arc::new(move |value| {
        let live = LIVE.load(Ordering::SeqCst);
        let mut marks = progress_marks.lock().unwrap();
        if value < 875 {
            marks.0 = Some(live);
        } else if marks.1.is_none() {
            marks.1 = Some(live);
        }
    }));
    let read = reader.read().expect("read dwg");
    assert_eq!(read.entities().count(), TEXTS);

    let (before, after) = *marks.lock().unwrap();
    let freed = before.expect("pass-2 progress") - after.expect("post-pass progress");
    // Every text is in the section once (twice the bytes from R2007 on, as
    // UTF-16), and the decoded copies stay on the document.
    let text_bytes = (TEXTS * TEXT_LEN) as isize;
    assert!(
        freed >= text_bytes,
        "only {freed} bytes freed between pass 2 and the post-pass; the section holds at least {text_bytes}"
    );
}
