// use color_eyre::Result;
use futures::{pin_mut, StreamExt};
use skystreamer::{stream::EventStream, RepoSubscription};

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create subscription to bsky.network
    let subscription = RepoSubscription::new("bsky.network").await.unwrap();
    // let subscription = repo;

    let mut binding = EventStream::new(subscription);
    let event_stream = binding.stream().await?;

    // let commit_stream = subscription.stream_commits().await;
    pin_mut!(event_stream);

    while let Some(record) = event_stream.next().await {
        // let c = Commit::from(&commit.unwrap());

        // let a = c.extract_records().await;

        // if !a.is_empty() {
        //     println!("{:?}", a);
        // }

        // println!("{:?}", record);

        if let skystreamer::types::commit::Record::Other(val) = record {
            println!("{:?}", val);
        }
    }

    Ok(())
}
