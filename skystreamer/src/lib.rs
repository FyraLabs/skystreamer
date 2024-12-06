// pub mod config;
pub mod stream;
pub mod types;
pub mod util;

use std::convert::Infallible;
pub const BLUESKY_FEED_DOMAIN: &str = "bsky.network";
use crate::types::Frame;
use atrium_api::{
    app::bsky::feed::Post as BPost,
    com::atproto::sync::subscribe_repos::{Commit, NSID},
    types::{CidLink, Collection},
};

use futures::StreamExt;

use ipld_core::ipld::Ipld;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use types::{operation::Operation, PostData, Subscription};
// use types::{CommitHandler, PostData, Subscription};
/// Error handling for the skystreamer crate
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
    _commit_cursor: u64,
    timeout: Option<tokio::time::Duration>,
}

impl RepoSubscription {
    pub async fn new(bgs: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        // todo: somehow get the websocket to update the damn params
        let request = format!("wss://{bgs}/xrpc/{NSID}").into_client_request()?;
        // request.
        let (stream, res) = connect_async(request).await?;
        tracing::debug!("Connected to websocket: {:?}", res);
        Ok(RepoSubscription {
            stream,
            _commit_cursor: 0,
            timeout: None,
        })
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
        let a = self.stream.get_config();
        tracing::debug!("Stream config: {:?}", a);
        futures::stream::unfold(self, |this| async move {
            loop {
                let timeout_duration = this
                    .timeout
                    .unwrap_or_else(|| tokio::time::Duration::from_secs(30));

                match tokio::time::timeout(timeout_duration, this.next()).await {
                    Ok(Some(Ok(Frame::Message(Some(t), message)))) if t.as_str() == "#commit" => {
                        // tracing::trace!("Received commit message: {:?}", message);
                        let commit = serde_ipld_dagcbor::from_reader(message.body.as_slice());
                        // tracing::trace!("Decoded commit: {:?}", commit);
                        return Some((commit.map_err(|e| e.into()), this));
                    }
                    Ok(Some(m)) => {
                        tracing::trace!("Unexpected message: {:?}", m);
                        continue;
                    }
                    Ok(None) => return None,
                    Err(elapsed) => {
                        tracing::warn!(?elapsed, "Timeout waiting for next message");
                        return None;
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

fn is_post_creation(op: &atrium_api::com::atproto::sync::subscribe_repos::RepoOp) -> bool {
    matches!(op.action.as_str(), "create") && op.path.split('/').next() == Some(BPost::NSID)
}

// convert a commit to
#[deprecated(note = "Please use [`Record::from_op`] instead.", since = "0.2.0")]
#[allow(deprecated)]
pub async fn handle_commit(commit: &Commit) -> Result<Vec<PostData>> {
    let mut posts = vec![];
    for op in &commit.ops {
        // let commit_type = op.path.split('/').next().unwrap_or_default();

        // todo: remove this check
        let commit_type = Operation::from_op(op.clone());
        tracing::debug!("New Commit type: {:?}", commit_type);

        if !is_post_creation(op) {
            // tracing::debug!("Skipping non-post creation op: {:?}", op);
            continue;
        }

        let record = types::commit::extract_post_record(op, &commit.blocks).await?;
        // posts.push(record.data);
        let post_data = PostData::new(commit.repo.clone(), commit.commit.clone(), record);
        posts.push(post_data);
    }

    Ok(posts)
}
