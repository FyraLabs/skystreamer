//! Helper types for bsky graph events

use crate::util::datetime_to_chrono;
use atrium_api::{
    app::bsky::{self},
    types::{string::Did, CidLink},
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

/// An event where someone blocks someone else :(
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub author: Did,
    pub subject: Did,
    pub created_at: DateTime<FixedOffset>,
    pub cid: Option<CidLink>,
}

impl BlockEvent {
    pub fn new(author: Did, record: bsky::graph::block::Record, cid: Option<CidLink>) -> Self {
        Self {
            author,
            created_at: datetime_to_chrono(&record.created_at),
            subject: record.subject.clone(),
            cid,
        }
    }
}

/// An event where someone follows someone else
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowEvent {
    pub author: Did,
    pub subject: Did,
    pub created_at: DateTime<FixedOffset>,
    pub cid: Option<CidLink>,
}

impl FollowEvent {
    pub fn new(author: Did, record: bsky::graph::follow::Record, cid: Option<CidLink>) -> Self {
        Self {
            author,
            created_at: datetime_to_chrono(&record.created_at),
            subject: record.subject.clone(),
            cid,
        }
    }
}

/// ListItem event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItemEvent {
    pub author: Did,
    pub subject: Did,
    pub created_at: DateTime<FixedOffset>,
    pub cid: Option<CidLink>,
    pub list: String,
}

impl ListItemEvent {
    pub fn new(author: Did, record: bsky::graph::listitem::Record, cid: Option<CidLink>) -> Self {
        Self {
            author,
            created_at: datetime_to_chrono(&record.created_at),
            subject: record.subject.clone(),
            cid,
            list: record.list.clone(),
        }
    }
}
