//! Helper types for deserialize commit data from the firehose.

use super::{actor::Profile, feed::*, graph::*, operation::Operation, Post};
use crate::{Error, Result};
use atrium_api::{
    app::bsky, com::atproto::sync::subscribe_repos::Commit as ACommit, types::CidLink,
};
use std::convert::From;

/// A record is an event that happens on ATProto.
/// It can be a post, or any kind of new event emitted from the network itself.
#[derive(Debug, Clone)]
pub enum Record {
    /// A new post
    Post(Box<Post>),
    /// A user blocks another user
    Block(Box<BlockEvent>),
    /// Someone likes a post
    Like(Box<LikeEvent>),
    /// Someone follows another user
    Follow(Box<FollowEvent>),
    /// Someone reposts a post
    Repost(Box<RepostEvent>),
    /// A new list item
    ListItem(Box<ListItemEvent>),
    /// A new user being created on the network
    Profile(Box<Profile>),
    // Other(Box<serde::de::value::>),
    /// Other, (yet) unsupported record types
    ///
    /// This is a catch-all for any record type that is not yet supported by the library.
    ///
    /// Returns a serde_json::Value, which can be used to inspect the raw data or manually
    /// deserialize it if needed.
    Other((Operation, Box<serde_json::Value>)),
}

impl Record {
    /// Deserialize an operation into the Record enum, given a commit.
    /// 
    /// Returns all the records that can be extracted from the operation.
    pub async fn from_op(op: &Operation, commit: &ACommit) -> Result<Vec<Self>> {
        let mut blocks = commit.blocks.as_slice();
        let mut records = vec![];
        let (items, _) = rs_car::car_read_all(&mut blocks, true).await?;
        let (_item_cid, item) = items
            .iter()
            .find(|(cid, _)| {
                let converted_cid = CidLink(
                    crate::types::CidOld::from(*cid)
                        .try_into()
                        .expect("invalid CID conversion"),
                );
                Some(converted_cid) == op.get_cid()
            })
            .ok_or_else(|| Error::ItemNotFound(op.get_cid(), items.len()))?;
        match op {
            Operation::Post(cidlink, _) => {
                records.push(Record::Post(Box::new(Post::from_record(
                    commit.repo.clone(),
                    cidlink.clone().unwrap(),
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?,
                ))));
            }
            Operation::Block(a, _) => {
                records.push(Record::Block(Box::new(BlockEvent::new(
                    commit.repo.clone(),
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?,
                    a.clone(),
                ))));
            }
            Operation::Like(link, _) => {
                records.push(Record::Like(Box::new(LikeEvent::new(
                    commit.repo.clone(),
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?,
                    link.clone(),
                ))));
            }
            Operation::Follow(link, _) => {
                // let follow: bsky::graph::follow::Record =
                //     serde_ipld_dagcbor::from_reader(&mut item.as_slice())?;

                records.push(Record::Follow(Box::new(FollowEvent::new(
                    commit.repo.clone(),
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?,
                    link.clone(),
                ))));
            }

            Operation::Repost(link, _) => {
                let repost: bsky::feed::repost::Record =
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?;
                records.push(Record::Repost(Box::new(RepostEvent::new(
                    commit.repo.clone(),
                    repost,
                    link.clone(),
                ))));
            }

            Operation::ListItem(link, _) => {
                records.push(Record::ListItem(Box::new(ListItemEvent::new(
                    commit.repo.clone(),
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?,
                    link.clone(),
                ))));
            }

            Operation::Profile(link, _) => {
                // todo

                let profile: bsky::actor::profile::Record =
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?;

                records.push(Record::Profile(Box::new(Profile::new(
                    commit.repo.clone(),
                    profile,
                    link.clone(),
                ))));
            }

            other => {
                tracing::trace!("Unhandled operation: {:?}", other);
                // todo: some kind of generic Serde value?
                records.push(Record::Other(*Box::new((
                    other.clone(),
                    serde_ipld_dagcbor::from_reader(&mut item.as_slice())?,
                ))));
            }
        }

        Ok(records)
    }
}


/// A singular commit, containing a list of operations.
/// 
/// This is a wrapper around [`atrium_api::com::atproto::sync::subscribe_repos::Commit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub operations: Vec<Operation>,
    inner_commit: ACommit,
}
impl From<&ACommit> for Commit {
    fn from(commit: &atrium_api::com::atproto::sync::subscribe_repos::Commit) -> Self {
        let ops = commit
            .ops
            .iter()
            .map(
                |op: &atrium_api::types::Object<
                    atrium_api::com::atproto::sync::subscribe_repos::RepoOpData,
                >| Operation::from_op(op.clone()),
            )
            .collect();

        Commit {
            operations: ops,
            inner_commit: commit.clone(),
        }
    }
}

/// Extract a post record from a commit.
#[deprecated(
    note = "Please use [`Record::from_op`] instead.",
    since = "0.2.0"
)]
pub async fn extract_post_record(
    op: &atrium_api::com::atproto::sync::subscribe_repos::RepoOp,
    mut blocks: &[u8],
) -> Result<bsky::feed::post::Record> {
    let (items, _) = rs_car::car_read_all(&mut blocks, true).await?;

    let (_, item) = items
        .iter()
        .find(|(cid, _)| {
            let converted_cid = CidLink(
                crate::types::CidOld::from(*cid)
                    .try_into()
                    .expect("invalid CID conversion"),
            );
            Some(converted_cid) == op.cid
        })
        .ok_or_else(|| Error::ItemNotFound(op.cid.clone(), items.len()))?;

    Ok(serde_ipld_dagcbor::from_reader(&mut item.as_slice())?)
}

impl Commit {

    /// Get the inner commit data, in case you need to access the raw commit.
    pub fn inner(&self) -> &ACommit {
        &self.inner_commit
    }

    /// Extracts all records from the commit.
    pub async fn extract_records(&self) -> Vec<Record> {
        let mut records = vec![];

        for op in &self.operations {
            let new_records = Record::from_op(op, &self.inner_commit).await;
            records.extend(new_records.unwrap_or_default());
        }
        records
    }
}
