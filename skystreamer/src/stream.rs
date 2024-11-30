//! Data export module for the bsky streamer.
//!
//! This module provides types, enums and functions for exporting data from the firehose.
//!
//!
use crate::types::Post;
use crate::Result;
use futures::StreamExt;
// a commit turns into a block of Posts
// is there a way to turn a block of posts into a stream of individual posts?

pub struct PostStream {
    // inner: Box<dyn futures::Stream<Item = Post> + Unpin + Send>,
    subscription: crate::RepoSubscription,
}

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
