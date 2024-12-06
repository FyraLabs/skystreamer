use super::{actor::Profile, feed::*, graph::*, operation::Operation, Post};
use crate::{Error, Result};
use atrium_api::{
    app::bsky, com::atproto::sync::subscribe_repos::Commit as ACommit, types::CidLink,
};
use std::convert::From;

#[derive(Debug, Clone)]
pub enum Record {
    Post(Box<Post>),
    Block(Box<BlockEvent>),
    Like(Box<LikeEvent>),
    Follow(Box<FollowEvent>),
    Repost(Box<RepostEvent>),
    ListItem(Box<ListItemEvent>),
    Profile(Box<Profile>),
    Other(String),
}

impl Record {
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
                // let post_record: bsky::feed::post::Record =
                //     serde_ipld_dagcbor::from_reader(&mut item.as_slice())?;
                // let post_record = bsky::feed::post::Record
                // let post_record = bsky::feed::post::Record::from(post_record);
                // records.push(Record::Post(post));
                // println!("{:?}", post_record);
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
                records.push(Record::Other(format!("{:?}", other)));
            }
        }

        Ok(records)
    }
}

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
    pub fn inner(&self) -> &ACommit {
        &self.inner_commit
    }

    pub async fn extract_records(&self) -> Vec<Record> {
        let mut records = vec![];

        for op in &self.operations {
            let new_records = Record::from_op(op, &self.inner_commit).await;
            records.extend(new_records.unwrap_or_default());
        }
        records
    }
}
