use atrium_api::types::string::{Cid as ACid, Datetime};
use chrono::{DateTime, FixedOffset};
use cid::Cid;

#[inline]
pub fn datetime_to_chrono(dt: &Datetime) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(dt.as_str()).unwrap()
}
#[inline]
pub fn conv_atrium_cid(cid: &ACid) -> Cid {
    Cid::from(cid.as_ref())
}
