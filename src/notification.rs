//! Parse notification / diagnostic system.
//!
//! Mirrors ACadSharp's `NotificationEventHandler` pattern.  Non-fatal issues
//! encountered during reading (or writing) are collected as `Notification`
//! items rather than being silently dropped or causing hard errors.
//!
//! After a read/write operation the caller can inspect
//! [`CadDocument::notifications`] to see what was encountered.

use std::fmt;

/// Severity level of a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationType {
    /// An entity/object/section is not yet implemented.
    NotImplemented,
    /// Feature exists but is not supported in this context.
    NotSupported,
    /// Non-fatal warning (e.g., missing handle, duplicate key).
    Warning,
    /// Error that was recovered from (e.g., bad group code value).
    Error,
}

impl fmt::Display for NotificationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => write!(f, "NotImplemented"),
            Self::NotSupported => write!(f, "NotSupported"),
            Self::Warning => write!(f, "Warning"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// A single notification produced during reading or writing.
#[derive(Debug, Clone)]
pub struct Notification {
    /// The severity / category.
    pub notification_type: NotificationType,
    /// A human-readable description of the issue.
    pub message: String,
}

impl Notification {
    /// Create a new notification.
    pub fn new(notification_type: NotificationType, message: impl Into<String>) -> Self {
        Self {
            notification_type,
            message: message.into(),
        }
    }
}

impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.notification_type, self.message)
    }
}

/// Collects notifications during a read/write operation.
#[derive(Debug, Clone, Default)]
pub struct NotificationCollection {
    items: Vec<Notification>,
}

impl NotificationCollection {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Record a notification.
    pub fn notify(&mut self, notification_type: NotificationType, message: impl Into<String>) {
        self.items.push(Notification::new(notification_type, message));
    }

    /// Check if there are any notifications.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of notifications.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterate over all notifications.
    pub fn iter(&self) -> std::slice::Iter<'_, Notification> {
        self.items.iter()
    }

    /// Get all notifications of a specific type.
    pub fn of_type(&self, nt: NotificationType) -> Vec<&Notification> {
        self.items.iter().filter(|n| n.notification_type == nt).collect()
    }

    /// Check whether any notification of the given type exists.
    pub fn has_type(&self, nt: NotificationType) -> bool {
        self.items.iter().any(|n| n.notification_type == nt)
    }

    /// Consume the collection into a `Vec`.
    pub fn into_vec(self) -> Vec<Notification> {
        self.items
    }
}

impl IntoIterator for NotificationCollection {
    type Item = Notification;
    type IntoIter = std::vec::IntoIter<Notification>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a NotificationCollection {
    type Item = &'a Notification;
    type IntoIter = std::slice::Iter<'a, Notification>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_creation() {
        let n = Notification::new(NotificationType::Warning, "handle missing");
        assert_eq!(n.notification_type, NotificationType::Warning);
        assert_eq!(n.message, "handle missing");
    }

    #[test]
    fn test_collection_basics() {
        let mut c = NotificationCollection::new();
        assert!(c.is_empty());

        c.notify(NotificationType::Warning, "w1");
        c.notify(NotificationType::Error, "e1");
        c.notify(NotificationType::Warning, "w2");

        assert_eq!(c.len(), 3);
        assert_eq!(c.of_type(NotificationType::Warning).len(), 2);
        assert!(c.has_type(NotificationType::Error));
        assert!(!c.has_type(NotificationType::NotImplemented));
    }

    #[test]
    fn test_display() {
        let n = Notification::new(NotificationType::NotImplemented, "THUMBNAILIMAGE section");
        assert_eq!(format!("{}", n), "[NotImplemented] THUMBNAILIMAGE section");
    }
}
