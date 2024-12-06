use atrium_api::types::string::{Cid as ACid, Datetime};
use chrono::{DateTime, FixedOffset};
use cid::Cid;

/// Convert an ATProto datetime to a chrono datetime
///
/// Simply dereferences the ATProto datetime.
#[inline]
pub fn datetime_to_chrono(dt: &Datetime) -> DateTime<FixedOffset> {
    *dt.as_ref()
}

/// Marshalls an older ATProto CID (CID 0.10) to a newer CID type (CID 0.11)
#[inline]
pub fn conv_atrium_cid(cid: &ACid) -> Cid {
    Cid::from(cid.as_ref())
}
