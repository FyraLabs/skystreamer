//! Data export module for the bsky streamer.
//!
//! This module provides types, enums and functions for exporting data from the firehose.
//!
//!
use crate::types::{commit, Post};
use crate::Result;
use futures::StreamExt;

#[deprecated(
    note = "Please use [`skystreamer::stream::EventStream`] instead as it provides a more generic interface.",
    since = "0.2.0"
)]
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

/// A helper for streaming events from the Firehose.
///
/// This struct wraps a [`crate::RepoSubscription`] and provides a stream of [`commit::Record`]s,
/// which can be used to export a [`futures::Stream`]-compatible stream of [`commit::Record`]s.
///
/// # Example
/// ```no_run
/// use futures::{pin_mut, StreamExt};
/// use skystreamer::{stream::EventStream, RepoSubscription};
///
/// let subscription = RepoSubscription::new("bsky.network").await.unwrap();
/// let mut binding = EventStream::new(subscription);
/// let event_stream = binding.stream().await?;
///
/// pin_mut!(event_stream);
/// // let's stream the data from the firehose!
/// while let Some(record) = event_stream.next().await {
///    // Outputs every known item in the stream
///    println!("{:?}", record);
/// }
/// ```
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

/// Simple helper function to create an [`EventStream`] from a domain directly.
///
/// ```no_run
/// use futures::{pin_mut, StreamExt};
/// use skystreamer::{stream::event_stream};
///
/// let mut event_stream = event_stream("bsky.network").await.unwrap();
/// let stream = event_stream.stream().await.unwrap();
///
/// pin_mut!(stream);
///
/// while let Some(record) = stream.next().await {
///     // do something with your data here
/// }
/// ```
pub async fn event_stream(domain: &str) -> EventStream {
    let subscription = crate::RepoSubscription::new(domain).await.unwrap();
    EventStream::new(subscription)
}
