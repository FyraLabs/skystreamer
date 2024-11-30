// use color_eyre::Result;
use futures::StreamExt;
use skystreamer::{stream::PostStream, RepoSubscription};

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create subscription to bsky.network
    let subscription = RepoSubscription::new("bsky.network").await.unwrap();
    // let subscription = repo;

    // Wrap in PostStream
    let post_stream = PostStream::new(subscription);
    let mut post_stream = post_stream.await;
    let stream = post_stream.stream().await?;

    // Pin the stream before processing
    futures::pin_mut!(stream);

    // Process posts as they arrive
    // should be Result<Post, Error>
    while let Some(post) = stream.next().await {
        println!("{:?}", post);
    }

    Ok(())
}
