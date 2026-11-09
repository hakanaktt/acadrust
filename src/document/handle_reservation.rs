//! Checked, monotonic reservation of CAD record identities.

use super::CadDocument;
use crate::{types::Handle, AuditLocation};

/// A failure to establish a safe tail range. No document state is changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandleReservationError {
    /// Record identities, aliases or storage indices disagree.
    #[error("inconsistent identity at {location:?}, {field_path} ({handle:?})")]
    InconsistentDocument {
        location: AuditLocation,
        field_path: String,
        handle: Option<Handle>,
        conflicting_location: Option<Box<AuditLocation>>,
    },
    /// A carrier has no qualified handle inventory.
    #[error("uninspectable handle carrier at {location:?}, {field_path}")]
    UninspectableData {
        location: AuditLocation,
        field_path: String,
    },
    /// No representable monotonic range fits this request.
    #[error("CAD handle tail range exhausted")]
    Exhausted,
}

/// Already consumed identities belonging to one document's allocation history.
///
/// Dropping this value does not release its numbers. It is not a portable
/// identity binding or evidence that later arbitrary document mutations are safe.
#[derive(Debug)]
#[must_use]
pub struct HandleReservation {
    start: u64,
    end: u64,
    count: usize,
}

impl HandleReservation {
    /// Number of reserved identities.
    pub fn len(&self) -> usize {
        self.count
    }
    /// Whether this is the no-op reservation.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Identity at a zero-based offset, without allocating a vector.
    pub fn get(&self, index: usize) -> Option<Handle> {
        if index >= self.count {
            return None;
        }
        Some(Handle::new(self.start + u64::try_from(index).ok()?))
    }
}

/// A lazy iterator over a consumed reservation.
#[derive(Debug)]
pub struct HandleReservationIter {
    next: u64,
    remaining: usize,
}

impl Iterator for HandleReservationIter {
    type Item = Handle;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let handle = Handle::new(self.next);
        self.next += 1;
        self.remaining -= 1;
        Some(handle)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for HandleReservationIter {}
impl std::iter::FusedIterator for HandleReservationIter {}
impl IntoIterator for HandleReservation {
    type Item = Handle;
    type IntoIter = HandleReservationIter;
    fn into_iter(self) -> Self::IntoIter {
        HandleReservationIter {
            next: self.start,
            remaining: self.count,
        }
    }
}

fn plan_range(
    next: u64,
    seed: u64,
    protected: u64,
    count: usize,
) -> Result<HandleReservation, HandleReservationError> {
    let protected_end = protected
        .checked_add(1)
        .ok_or(HandleReservationError::Exhausted)?;
    let start = 1.max(next).max(seed).max(protected_end);
    let width = u64::try_from(count).map_err(|_| HandleReservationError::Exhausted)?;
    let end = start
        .checked_add(width)
        .ok_or(HandleReservationError::Exhausted)?;
    Ok(HandleReservation { start, end, count })
}

impl CadDocument {
    /// Reserve a batch above every qualified identity/reference and both floors.
    ///
    /// Intended for documents assembled in memory from supported typed records.
    /// Raw backing retained by DWG or DXF readers can cause a positive request
    /// to return [`HandleReservationError::UninspectableData`].
    ///
    /// A zero count returns empty without inspecting or certifying the document.
    /// Positive requests inspect current storage and reject unqualified carriers.
    /// On error all document state is unchanged; on success only the private
    /// allocator and header HANDSEED advance. Existing allocator APIs are unchanged.
    pub fn try_reserve_handles(
        &mut self,
        count: usize,
    ) -> Result<HandleReservation, HandleReservationError> {
        if count == 0 {
            return Ok(HandleReservation {
                start: 0,
                end: 0,
                count: 0,
            });
        }
        let protected = super::handle_usage::census(self)?;
        let reservation = plan_range(self.next_handle, self.header.handle_seed, protected, count)?;
        self.next_handle = reservation.end;
        self.header.handle_seed = reservation.end;
        Ok(reservation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_reservation_checked_tail_boundaries() {
        assert!(plan_range(1, 1, u64::MAX, 1).is_err());
        assert!(plan_range(1, 1, u64::MAX - 1, 1).is_err());
        let r = plan_range(u64::MAX - 1, 1, 0, 1).unwrap();
        assert_eq!(r.get(0).unwrap().value(), u64::MAX - 1);
        assert_eq!(r.end, u64::MAX);
        assert!(plan_range(r.end, r.end, 0, 1).is_err());
        assert_eq!(plan_range(0, 0, 0, 1).unwrap().get(0).unwrap().value(), 1);
    }

    #[test]
    fn handle_reservation_range_is_full_width_and_lazy() {
        let first = (1u64 << 53) + 17;
        let r = plan_range(first, 1, 0, 3).unwrap();
        assert_eq!(r.len(), 3);
        assert!(!r.is_empty());
        assert_eq!(r.get(0).unwrap().value(), first);
        assert_eq!(r.get(2).unwrap().value(), first + 2);
        assert_eq!(r.get(3), None);
        assert_eq!(r.get(usize::MAX), None);
        let mut it = r.into_iter();
        assert_eq!(it.len(), 3);
        assert_eq!(it.next().unwrap().value(), first);
        assert_eq!(it.len(), 2);
        assert_eq!(it.count(), 2);
        #[cfg(target_pointer_width = "64")]
        assert!(plan_range(1, 1, 0, usize::MAX).is_err());
        #[cfg(target_pointer_width = "32")]
        assert_eq!(plan_range(1, 1, 0, usize::MAX).unwrap().len(), usize::MAX);
    }

    #[test]
    fn handle_reservation_never_lowers_floors() {
        assert_eq!(
            plan_range(400, 800, 100, 2)
                .unwrap()
                .get(0)
                .unwrap()
                .value(),
            800
        );
        assert_eq!(
            plan_range(800, 400, 100, 2)
                .unwrap()
                .get(0)
                .unwrap()
                .value(),
            800
        );
        assert_eq!(
            plan_range(1, 1, 900, 2).unwrap().get(0).unwrap().value(),
            901
        );
    }
}
