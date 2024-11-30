// pub mod config;
pub mod stream;
pub mod types;

use std::convert::Infallible;
pub const BLUESKY_FEED_DOMAIN: &str = "bsky.network";
use crate::types::Frame;
use atrium_api::{
    app::bsky::feed::{post::Record, Post as BPost},
    com::atproto::sync::subscribe_repos::{Commit, NSID},
    types::{CidLink, Collection},
};

use futures::StreamExt;

use ipld_core::ipld::Ipld;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use types::{PostData, Subscription};
// use types::{CommitHandler, PostData, Subscription};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to connect to websocket: {0}")]
    Connect(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Failed to decide CBOR: {0}")]
    CborDecoder(#[from] serde_ipld_dagcbor::DecodeError<std::io::Error>),
    #[error("Failed to decode CBOR (How!?): {0}")]
    CborDecode(#[from] serde_ipld_dagcbor::DecodeError<Infallible>),
    #[error("Failed to decode CAR data: {0}")]
    CarDecoder(#[from] rs_car::CarDecodeError),
    #[error("Could not find item with operation cid {0:?} out of {1} items")]
    ItemNotFound(Option<CidLink>, usize),
    #[error("Invalid frame data: {0:?}")]
    InvalidFrameData(Vec<u8>),
    #[error("Invalid frame type: {0:?}")]
    InvalidFrameType(Ipld),
    #[error("ATrium error: {0}")]
    AtriumError(String),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub struct RepoSubscription {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl RepoSubscription {
    pub async fn new(bgs: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let (stream, _) = connect_async(format!("wss://{bgs}/xrpc/{NSID}")).await?;
        Ok(RepoSubscription { stream })
    }

    // pub async fn run(
    //     &mut self,
    //     mut handler: impl CommitHandler,
    // ) -> std::result::Result<(), Box<dyn std::error::Error>> {
    //     let mut commit_count = 0;
    //     while let Some(result) = self.next().await {
    //         if let Ok(Frame::Message(Some(t), message)) = result {
    //             if t.as_str() == "#commit" {
    //                 let commit = serde_ipld_dagcbor::from_reader(message.body.as_slice())?;
    //                 if let Err(err) = handler.handle_commit(&commit).await {
    //                     tracing::error!("FAILED: {err:?}");
    //                 }
    //                 commit_count += 1;
    //                 if commit_count >= 20 {
    //                     // Update cursor logic here
    //                     // Assuming `update_cursor` is a method on the handler
    //                     handler.update_cursor(commit.seq as u64).await?;
    //                     commit_count = 0;
    //                 }
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    pub async fn stream_commits(
        &mut self,
    ) -> impl futures::Stream<Item = std::result::Result<Commit, Box<dyn std::error::Error>>> + '_
    {
        futures::stream::unfold(self, |this| async move {
            loop {
                if let Some(Ok(Frame::Message(Some(t), message))) = this.next().await {
                    if t.as_str() == "#commit" {
                        let commit = serde_ipld_dagcbor::from_reader(message.body.as_slice());
                        return Some((commit.map_err(|e| e.into()), this));
                    }
                }
            }
        })
    }
}

impl Subscription for RepoSubscription {
    async fn next(&mut self) -> Option<Result<Frame>> {
        if let Some(Ok(Message::Binary(data))) = self.stream.next().await {
            Some(Frame::try_from(data.as_slice()))
        } else {
            None
        }
    }
}

pub async fn extract_post_record(
    op: &atrium_api::com::atproto::sync::subscribe_repos::RepoOp,
    mut blocks: &[u8],
) -> Result<Record> {
    let (items, _) = rs_car::car_read_all(&mut blocks, true).await?;

    let (_, item) = items
        .iter()
        .find(|(cid, _)| {
            let converted_cid = CidLink(
                types::CidOld::from(*cid)
                    .try_into()
                    .expect("invalid CID conversion"),
            );
            Some(converted_cid) == op.cid
        })
        .ok_or_else(|| {
            // eyre!(
            //     "Could not find item with operation cid {:?} out of {} items",
            //     op.cid,
            //     items.len()
            // )

            Error::ItemNotFound(op.cid.clone(), items.len())
        })?;

    Ok(serde_ipld_dagcbor::from_reader(&mut item.as_slice())?)
}

fn is_post_creation(op: &atrium_api::com::atproto::sync::subscribe_repos::RepoOp) -> bool {
    matches!(op.action.as_str(), "create") && op.path.split('/').next() == Some(BPost::NSID)
}
// convert a commit to
pub async fn handle_commit(commit: &Commit) -> Result<Vec<PostData>> {
    let mut posts = vec![];
    for op in &commit.ops {
        if !is_post_creation(op) {
            continue;
        }

        let record = extract_post_record(op, &commit.blocks).await?;
        // posts.push(record.data);
        let post_data = PostData::new(commit.repo.clone(), commit.commit.clone(), record);
        posts.push(post_data);
    }

    Ok(posts)
}
