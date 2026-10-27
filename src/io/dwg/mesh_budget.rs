//! Host-scaled cap for the faces stored from one DWG MESH record.
//!
//! The reader used to stop at a fixed 250,000 faces and 1,000,000 face-corner
//! indices. Dense but legitimate meshes were truncated. The cap now follows
//! installed and available physical memory, and [`crate::DwgReadOptions`] can
//! replace either limit. `usize::MAX` removes a cap. A face list larger than
//! the bytes left in the object record is still rejected.

/// Faces and face-corner indices retained from a single MESH record.
///
/// The system budget is one eighth of installed RAM and at most one quarter of
/// currently available RAM, whichever is smaller. That byte budget is turned
/// into counts using 96 bytes of `Vec` overhead per triangular face plus 48
/// bytes per corner (the temporary `i32`, the stored `usize`, and allocator
/// slack). Hosts that cannot report memory, or that would otherwise land below
/// the historical caps, keep [`MeshFaceBudget::BASELINE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshFaceBudget {
    /// Maximum stored faces. [`usize::MAX`] disables the cap.
    pub max_faces: usize,
    /// Maximum stored face-corner indices. [`usize::MAX`] disables the cap.
    pub max_face_indices: usize,
}

impl MeshFaceBudget {
    /// Historical fixed caps. Also the floor of every system-derived budget.
    pub const BASELINE: Self = Self {
        max_faces: 250_000,
        max_face_indices: 1_000_000,
    };

    /// No artificial cap. The object record still bounds the face list.
    pub const UNLIMITED: Self = Self {
        max_faces: usize::MAX,
        max_face_indices: usize::MAX,
    };

    /// Budget from a fresh probe of physical memory.
    ///
    /// Falls back to [`Self::BASELINE`] when the host exposes no counter, which
    /// is typical for `wasm32`. A [`crate::DwgReader`] read samples once per file.
    pub fn from_system() -> Self {
        let (total, available) = probe_host_memory();
        Self::from_host_memory(total, available)
    }

    /// Resolve a budget from installed and available physical memory, in bytes.
    ///
    /// `None` or `Some(0)` means that figure is unknown. When both are known
    /// the budget is `min(total / 8, available / 4)`.
    pub fn from_host_memory(total_bytes: Option<u64>, available_bytes: Option<u64>) -> Self {
        let total = total_bytes.filter(|value| *value > 0);
        let available = available_bytes.filter(|value| *value > 0);
        let budget = match (total, available) {
            (Some(total), Some(available)) => (total / 8).min(available / 4),
            (None, Some(available)) => available / 4,
            (Some(total), None) => total / 8,
            (None, None) => return Self::BASELINE,
        };
        Self::from_budget_bytes(budget)
    }

    /// Replace either limit. `None` keeps the value already on `self`.
    pub fn with_overrides(self, max_faces: Option<usize>, max_face_indices: Option<usize>) -> Self {
        Self {
            max_faces: max_faces.unwrap_or(self.max_faces),
            max_face_indices: max_face_indices.unwrap_or(self.max_face_indices),
        }
    }

    /// Whether another face still fits in both caps.
    ///
    /// A face is kept whole or not at all. Index addition that would overflow
    /// `usize` does not fit.
    pub fn allows_face(
        &self,
        stored_faces: usize,
        stored_indices: usize,
        face_vertices: usize,
    ) -> bool {
        stored_faces < self.max_faces
            && stored_indices
                .checked_add(face_vertices)
                .is_some_and(|count| count <= self.max_face_indices)
    }

    fn from_budget_bytes(budget: u64) -> Self {
        const BYTES_PER_INDEX: u64 = 48;
        const BYTES_PER_FACE: u64 = 96 + 3 * BYTES_PER_INDEX;
        if budget < BYTES_PER_FACE {
            return Self::BASELINE;
        }
        let faces = usize::try_from(budget / BYTES_PER_FACE).unwrap_or(usize::MAX);
        let indices = usize::try_from(budget / BYTES_PER_INDEX).unwrap_or(usize::MAX);
        Self {
            max_faces: faces.max(Self::BASELINE.max_faces),
            max_face_indices: indices.max(Self::BASELINE.max_face_indices),
        }
    }
}

fn probe_host_memory() -> (Option<u64>, Option<u64>) {
    #[cfg(windows)]
    {
        return windows_memory();
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        return linux_memory();
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        return (unix_physical_bytes(), None);
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    {
        (None, None)
    }
}

#[cfg(windows)]
fn windows_memory() -> (Option<u64>, Option<u64>) {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut status = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    // SAFETY: `status.length` is the size of this `MEMORYSTATUSEX` layout
    // (two `DWORD`s followed by seven `DWORDLONG`s, 64 bytes, 8-aligned).
    // The call only writes into that object.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        return (None, None);
    }
    (Some(status.total_phys), Some(status.avail_phys))
}

#[cfg(any(test, target_os = "linux", target_os = "android"))]
fn memory_from_meminfo(text: &str) -> (Option<u64>, Option<u64>) {
    let total = meminfo_kib(text, "MemTotal:").map(|kib| kib.saturating_mul(1024));
    let available = meminfo_kib(text, "MemAvailable:")
        .or_else(|| meminfo_kib(text, "MemFree:"))
        .map(|kib| kib.saturating_mul(1024));
    (total, available)
}

#[cfg(any(test, target_os = "linux", target_os = "android"))]
fn meminfo_kib(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix(key)?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn linux_memory() -> (Option<u64>, Option<u64>) {
    match std::fs::read_to_string("/proc/meminfo") {
        Ok(text) => memory_from_meminfo(&text),
        Err(_) => (None, None),
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn unix_physical_bytes() -> Option<u64> {
    sysctl_u64(c"hw.memsize").or_else(|| sysctl_u64(c"hw.physmem"))
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn sysctl_u64(name: &std::ffi::CStr) -> Option<u64> {
    extern "C" {
        fn sysctlbyname(
            name: *const std::ffi::c_char,
            oldp: *mut std::ffi::c_void,
            oldlenp: *mut usize,
            newp: *const std::ffi::c_void,
            newlen: usize,
        ) -> i32;
    }

    let mut value = 0u64;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: `name` is NUL-terminated, `value` is a live u64, and `len` is
    // its size. The kernel writes at most `len` bytes and does not keep the
    // pointers.
    let rc = unsafe {
        sysctlbyname(
            name.as_ptr(),
            &mut value as *mut u64 as *mut std::ffi::c_void,
            &mut len,
            std::ptr::null(),
            0,
        )
    };
    if rc != 0 || len == 0 || len > std::mem::size_of::<u64>() {
        return None;
    }
    let bytes = value.to_ne_bytes();
    let mut wide = [0u8; 8];
    wide[..len].copy_from_slice(&bytes[..len]);
    Some(u64::from_ne_bytes(wide)).filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_or_tiny_hosts_keep_the_historical_floor() {
        assert_eq!(
            MeshFaceBudget::from_host_memory(None, None),
            MeshFaceBudget::BASELINE
        );
        assert_eq!(
            MeshFaceBudget::from_host_memory(Some(0), Some(0)),
            MeshFaceBudget::BASELINE
        );
        assert_eq!(
            MeshFaceBudget::from_host_memory(Some(64 << 20), Some(64 << 20)),
            MeshFaceBudget::BASELINE
        );
        let pressured = MeshFaceBudget::from_host_memory(Some(16 << 30), Some(32 << 20));
        assert_eq!(pressured, MeshFaceBudget::BASELINE);
    }

    #[test]
    fn larger_hosts_scale_both_caps_from_the_tighter_share() {
        let wide = MeshFaceBudget::from_host_memory(Some(16 << 30), Some(12 << 30));
        assert_eq!(wide.max_faces, (2 << 30) / 240);
        assert_eq!(wide.max_face_indices, (2 << 30) / 48);
        assert!(wide.max_faces > MeshFaceBudget::BASELINE.max_faces);
        assert!(wide.max_face_indices > MeshFaceBudget::BASELINE.max_face_indices);

        let total_only = MeshFaceBudget::from_host_memory(Some(8 << 30), None);
        assert_eq!(total_only.max_faces, (1 << 30) / 240);
        assert_eq!(total_only.max_face_indices, (1 << 30) / 48);

        let available_only = MeshFaceBudget::from_host_memory(None, Some(8 << 30));
        assert_eq!(available_only.max_faces, (2 << 30) / 240);
        assert_eq!(available_only.max_face_indices, (2 << 30) / 48);
    }

    #[test]
    fn overrides_replace_one_side_and_unlimited_disables_both() {
        let budget = MeshFaceBudget::BASELINE.with_overrides(Some(9), None);
        assert_eq!(budget.max_faces, 9);
        assert_eq!(
            budget.max_face_indices,
            MeshFaceBudget::BASELINE.max_face_indices
        );
        assert_eq!(
            budget.with_overrides(Some(usize::MAX), Some(usize::MAX)),
            MeshFaceBudget::UNLIMITED
        );
    }

    #[test]
    fn a_face_must_fit_both_caps_without_overflow() {
        let budget = MeshFaceBudget {
            max_faces: 2,
            max_face_indices: 7,
        };
        assert!(budget.allows_face(0, 0, 3));
        assert!(budget.allows_face(1, 3, 3));
        assert!(!budget.allows_face(2, 6, 3));
        assert!(!budget.allows_face(1, 6, 3));
        assert!(MeshFaceBudget::UNLIMITED.allows_face(usize::MAX - 1, usize::MAX - 3, 3));
        assert!(!MeshFaceBudget::UNLIMITED.allows_face(usize::MAX - 1, usize::MAX - 2, 3));
    }

    #[test]
    fn meminfo_prefers_available_and_falls_back_to_free() {
        let text = "\
MemTotal:       16384 kB
MemFree:         1024 kB
MemAvailable:    8192 kB
";
        assert_eq!(
            memory_from_meminfo(text),
            (Some(16_384 * 1024), Some(8_192 * 1024))
        );
        let free_only = "MemTotal: 10 kB\nMemFree: 4 kB\n";
        assert_eq!(
            memory_from_meminfo(free_only),
            (Some(10 * 1024), Some(4 * 1024))
        );
        assert_eq!(memory_from_meminfo("nonsense"), (None, None));
    }

    #[test]
    fn system_budget_is_at_least_the_floor() {
        let budget = MeshFaceBudget::from_system();
        assert!(budget.max_faces >= MeshFaceBudget::BASELINE.max_faces);
        assert!(budget.max_face_indices >= MeshFaceBudget::BASELINE.max_face_indices);
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        {
            let (total, available) = probe_host_memory();
            assert!(
                total.unwrap_or(0).max(available.unwrap_or(0)) > 0,
                "host memory probe returned {total:?} {available:?}"
            );
        }
    }
}
