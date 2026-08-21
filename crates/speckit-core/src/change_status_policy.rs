/// Change status policy - determines how changes transition between states.

/// Status of a change
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeStatus {
    /// Change is being worked on
    Active,
    /// Change is ready for review
    Ready,
    /// Change has been archived
    Archived,
}

/// Check if a change can be archived.
pub fn can_archive(status: &ChangeStatus) -> bool {
    matches!(status, ChangeStatus::Active | ChangeStatus::Ready)
}

/// Check if a change can be validated.
pub fn can_validate(status: &ChangeStatus) -> bool {
    matches!(status, ChangeStatus::Active | ChangeStatus::Ready)
}
