//! Idiomatic Rust from the Bluesky API actor types
use crate::util::{conv_atrium_cid, datetime_to_chrono};
use atrium_api::{
    app::bsky::{self},
    types::{string::Did, CidLink, Union},
};
use chrono::{DateTime, FixedOffset};
use cid::Cid;
use serde::{Deserialize, Serialize};

use super::Blob;

/// A user profile
///
/// A profile is, simply put, an account on Bluesky Social.
///
/// It contains information about the user, such as their display name, avatar, and
/// bio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// The ATProto DID (Repo ID) of the user
    pub did: Did,
    /// Link to the user's profile image, if any
    pub avatar: Option<Blob>,
    /// The date the profile was created
    pub created_at: Option<DateTime<FixedOffset>>,
    /// The user's bio text, if any
    pub description: Option<String>,
    /// The user's display name (not their handle)
    pub display_name: Option<String>,
    // pub joined_via_starter_pack: Option<crate::com::atproto::repo::strong_ref::Main>,
    /// Self-imposed labels this user has put on themselves
    pub labels: Vec<String>,
    /// A link to the user's pinned post on their profile,
    /// Refers to [`crate::types::Post`]
    pub pinned_post: Option<Cid>,
}

impl Profile {
    /// Create a new profile from their record stream.
    /// Used in the [`crate::stream::EventStream`] to create a profile from a commit.
    pub fn new(did: Did, record: bsky::actor::profile::Record, cid: Option<CidLink>) -> Self {
        if let Some(cid) = cid {
            tracing::trace!("Profile cid: {:?}", cid);
        }

        Self {
            did,
            avatar: record.avatar.clone().map(Blob::from),
            created_at: record.created_at.as_ref().map(datetime_to_chrono),
            description: record.description.clone(),
            display_name: record.display_name.clone(),
            // joined_via_starter_pack: record.joined_via_starter_pack.clone(),
            labels: record
                .labels.clone()
                // holy unions
                .map(|l| {
                    if let Union::Refs(atrium_api::app::bsky::actor::profile::RecordLabelsRefs::ComAtprotoLabelDefsSelfLabels(data)) = l {
                        data.data.values.iter().map(|v| v.val.clone()).collect()
                    } else {
                        Vec::new()
                    }
                })
                .unwrap_or_default(),
            pinned_post: record.pinned_post.as_ref().map(|p| conv_atrium_cid(&p.cid)),
        }
    }
}
