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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub did: Did,
    pub avatar: Option<Blob>,
    pub created_at: Option<DateTime<FixedOffset>>,
    pub description: Option<String>,
    pub display_name: Option<String>,
    // pub joined_via_starter_pack: Option<crate::com::atproto::repo::strong_ref::Main>,
    pub labels: Vec<String>,
    pub pinned_post: Option<Cid>,
}

impl Profile {
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
