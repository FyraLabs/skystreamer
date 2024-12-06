//! Helper types for bsky graph events

use crate::util::{conv_atrium_cid, datetime_to_chrono};
use atrium_api::{
    app::bsky,
    types::{string::Did, CidLink},
};
use chrono::{DateTime, FixedOffset};
use cid::Cid;
use serde::{Deserialize, Serialize};

/// An event where someone likes a post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikeEvent {
    pub author: Did,
    pub subject: Cid,
    pub created_at: DateTime<FixedOffset>,
    pub cid: Option<CidLink>,
}

impl LikeEvent {
    pub fn new(author: Did, record: bsky::feed::like::Record, cid: Option<CidLink>) -> Self {
        // let subject = record.subject.cid.as_ref();

        Self {
            author,
            created_at: datetime_to_chrono(&record.created_at),
            subject: conv_atrium_cid(&record.subject.cid),
            cid,
        }
    }
}

/// An event where someone reposts a post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepostEvent {
    pub author: Did,
    pub subject: Cid,
    pub created_at: DateTime<FixedOffset>,
    pub cid: Option<CidLink>,
}

impl RepostEvent {
    pub fn new(author: Did, record: bsky::feed::repost::Record, cid: Option<CidLink>) -> Self {
        Self {
            author,
            created_at: datetime_to_chrono(&record.created_at),
            subject: conv_atrium_cid(&record.subject.cid),
            cid,
        }
    }
}
