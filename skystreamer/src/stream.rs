//! Data export module for the bsky streamer.
//!
//! This module provides types, enums and functions for exporting data from the firehose.
//!
//!
use crate::types::{commit, Post};
use crate::Result;
use futures::StreamExt;

#[deprecated(note = "Please use [`EventStream`] instead")]
pub struct PostStream {
    // inner: Box<dyn futures::Stream<Item = Post> + Unpin + Send>,
    subscription: crate::RepoSubscription,
}

#[allow(deprecated)]
impl PostStream {
    pub async fn new(inner: crate::RepoSubscription) -> Self {
        PostStream {
            subscription: inner,
        }
    }

    pub async fn stream(&mut self) -> Result<impl futures::Stream<Item = Post> + '_> {
        let block_stream = self.subscription.stream_commits();

        let stream = block_stream
            .await
            .filter_map(|result| async {
                match result {
                    Ok(commit) => {
                        let posts = crate::handle_commit(&commit).await.ok().map(|posts| {
                            posts
                                .iter()
                                .map(|post| Post::from(post.clone()))
                                .collect::<Vec<_>>()
                        });
                        posts.map(futures::stream::iter)
                    }
                    Err(e) => {
                        tracing::error!("Error processing commit: {}", e);
                        None
                    }
                }
            })
            .flatten();
        Ok(stream)
    }
}

/// A stream of every event from the firehose.
/// Replaces the old [`PostStream`] type.
/// 
/// This stream will yield every single event from the firehose, 
pub struct EventStream {
    subscription: crate::RepoSubscription,
}

impl EventStream {
    pub fn new(inner: crate::RepoSubscription) -> Self {
        EventStream {
            subscription: inner,
        }
    }

    pub async fn stream(&mut self) -> Result<impl futures::Stream<Item = commit::Record> + '_> {
        let block_stream = self.subscription.stream_commits();

        let stream = block_stream
            .await
            .filter_map(|result| async {
                match result {
                    Ok(commit_data) => {
                        let commit = commit::Commit::from(&commit_data);
                        let records = commit.extract_records().await;
                        Some(futures::stream::iter(records.into_iter()))
                    }
                    Err(e) => {
                        tracing::error!("Error processing commit: {}", e);
                        None
                    }
                }
            })
            .flatten();
        Ok(stream)
    }
}
