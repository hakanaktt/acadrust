//! Handles and offsets come from the file, so the maps keyed by them must not
//! hash under a seed a file can be crafted against in advance.

use super::*;
use crate::io::dwg::dwg_stream_readers::handle_reader::read_handle_map;
use std::hash::{BuildHasher, Hash};
use std::sync::OnceLock;

/// More low hash bits than these keys need to share to pick the same bucket
/// in a table of a few thousand slots.
const LOW_BITS: u64 = 0xFFF;
const KEYS: usize = 1_000;
/// 1,000 keys over 4,096 slots, spread at random, never come close to this.
const CLUSTERED: usize = 100;

/// Keys whose low hash bits are all equal under foldhash's public fixed seed,
/// found the way an attacker would: by trying keys until enough collide.
fn crafted_keys() -> &'static [u64] {
    static KEYS_FOUND: OnceLock<Vec<u64>> = OnceLock::new();
    KEYS_FOUND.get_or_init(|| {
        let fixed = foldhash::fast::FixedState::default();
        (1u64..)
            .filter(|key| fixed.hash_one(key) & LOW_BITS == 0)
            .take(KEYS)
            .collect()
    })
}

/// The most keys sharing one bucket under `state`.
fn fullest_bucket<K: Hash, S: BuildHasher>(state: &S, keys: impl Iterator<Item = K>) -> usize {
    let mut buckets = vec![0usize; LOW_BITS as usize + 1];
    for key in keys {
        buckets[(state.hash_one(&key) & LOW_BITS) as usize] += 1;
    }
    buckets.into_iter().max().unwrap_or(0)
}

fn handles() -> impl Iterator<Item = Handle> {
    crafted_keys().iter().map(|&key| Handle::new(key))
}

/// A handle section holding `keys`, ascending. The reader stops at a chunk
/// over 2,048 bytes and starts each chunk's deltas from zero.
fn handle_section(keys: &[u64]) -> Vec<u8> {
    let mut section = Vec::new();
    for keys in keys.chunks(200) {
        let mut chunk = Vec::new();
        let mut previous = 0;
        for &key in keys {
            let mut delta = key - previous;
            previous = key;
            while delta >= 0x80 {
                chunk.push((delta & 0x7F) as u8 | 0x80);
                delta >>= 7;
            }
            chunk.push(delta as u8);
            chunk.push(0x01);
        }
        let size = u16::try_from(chunk.len() + 2).unwrap();
        section.extend_from_slice(&size.to_be_bytes());
        section.append(&mut chunk);
        section.extend_from_slice(&[0, 0]);
    }
    section
}

#[test]
fn a_crafted_handle_section_does_not_cluster_in_the_handle_map() {
    let map = read_handle_map(&handle_section(crafted_keys())).unwrap();
    assert_eq!(map.len(), KEYS);
    let fullest = fullest_bucket(map.hasher(), map.keys());
    assert!(
        fullest < CLUSTERED,
        "{fullest} of {KEYS} keys in one bucket"
    );
}

#[test]
fn crafted_handles_do_not_cluster_in_the_document_maps() {
    // `Handle` hashes as its `u64`, so the same keys aim at these maps too.
    assert_eq!(
        fullest_bucket(&foldhash::fast::FixedState::default(), handles()),
        KEYS
    );
    let doc = CadDocument::new();
    let keys = || crafted_keys().iter();
    for (name, fullest) in [
        (
            "entity_index",
            fullest_bucket(doc.entity_index.hasher(), handles()),
        ),
        (
            "eed_by_handle",
            fullest_bucket(doc.eed_by_handle.hasher(), handles()),
        ),
        (
            "xdic_by_handle",
            fullest_bucket(doc.xdic_by_handle.hasher(), handles()),
        ),
        (
            "reactors_by_handle",
            fullest_bucket(doc.reactors_by_handle.hasher(), handles()),
        ),
        (
            "block_entity_handles",
            fullest_bucket(doc.block_entity_handles.hasher(), handles()),
        ),
        (
            "original_objects",
            fullest_bucket(doc.original_objects.hasher(), handles()),
        ),
        (
            "raw_records",
            fullest_bucket(doc.raw_records.hasher(), keys()),
        ),
        (
            "raw_record_owners",
            fullest_bucket(doc.raw_record_owners.hasher(), keys()),
        ),
    ] {
        assert!(
            fullest < CLUSTERED,
            "{name}: {fullest} of {KEYS} keys in one bucket"
        );
    }
}

#[test]
fn two_handle_maps_do_not_share_a_seed() {
    let first = read_handle_map(&[]).unwrap();
    let second = read_handle_map(&[]).unwrap();
    assert!(crafted_keys()
        .iter()
        .any(|key| first.hasher().hash_one(key) != second.hasher().hash_one(key)));
}

#[test]
fn a_cloned_map_keeps_its_seed_and_finds_its_keys() {
    let map = read_handle_map(&handle_section(crafted_keys())).unwrap();
    let clone = map.clone();
    for key in crafted_keys() {
        assert_eq!(clone.hasher().hash_one(key), map.hasher().hash_one(key));
        assert_eq!(clone.get(key), map.get(key));
    }
}
