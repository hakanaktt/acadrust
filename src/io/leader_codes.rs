//! Report unknown source codes without changing the existing enum conversions.
use super::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use super::read::{push_read_diagnostic, ReadDiagnostic, ReadStage, MAX_READ_DIAGNOSTICS};
use crate::notification::{NotificationCollection, NotificationType};

pub(crate) fn warning(value: i32, allowed: &[i16], field: &str) -> Option<ReadDiagnostic> {
    (!allowed.iter().any(|&code| i32::from(code) == value)).then(|| {
        ReadDiagnostic::new(
            "unknown-leader-enum",
            ReadStage::RecordStream,
            format!("Unknown {field} value {value}; using the existing enum conversion"),
        )
    })
}

pub(crate) fn dwg(reader: &mut DwgMergedReader, value: i16, allowed: &[i16], field: &str) -> i16 {
    if let Some(warning) = warning(i32::from(value), allowed, field) {
        reader.push_leader_diagnostic(warning);
    }
    value
}

pub(crate) fn report(
    notifications: &mut NotificationCollection,
    diagnostics: &mut Vec<ReadDiagnostic>,
    warning: ReadDiagnostic,
) {
    if diagnostics.len() < MAX_READ_DIAGNOSTICS.saturating_sub(1) {
        notifications.notify(NotificationType::Warning, warning.message.clone());
    }
    push_read_diagnostic(diagnostics, warning);
}
