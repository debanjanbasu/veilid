use super::*;

use rkyv::{Archive as RkyvArchive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::*;

/// Information required to handle locally opened records
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RkyvArchive, RkyvSerialize, RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct LocalRecordDetail {
    /// The last 'safety selection' used when creating/opening this record.
    /// Even when closed, this safety selection applies to republication attempts by the system.
    safety_selection: SafetySelection,
}
