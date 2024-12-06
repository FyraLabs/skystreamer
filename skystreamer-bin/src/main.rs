// use anyhow::{anyhow, Result};
// use crate::types::Frame;
mod config;
mod exporter;
mod surreal_types;
use clap::Parser;
use color_eyre::Result;
use futures::StreamExt;
use skystreamer::{stream::EventStream, RepoSubscription};
// use std::sync::Arc;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use update_rate::RateCounter;

pub struct Consumer {
    rate_counter: update_rate::DiscreteRateCounter,
    exporter: Box<dyn exporter::Exporter>,
    pub atproto_relay: String,
}

impl Consumer {
    pub fn new(exporter: Box<dyn exporter::Exporter>, relay: &str) -> Self {
        Consumer {
            rate_counter: update_rate::DiscreteRateCounter::new(50),
            exporter,
            atproto_relay: relay.to_string(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let subscription = RepoSubscription::new(&self.atproto_relay).await.unwrap();
        let mut event_stream = EventStream::new(subscription);
        let stream = event_stream.stream().await?;

        // let post_stream = PostStream::new(subscription);

        // let mut post_stream = post_stream.await;

        futures::pin_mut!(stream);

        while let Some(skystreamer::types::commit::Record::Post(post)) = stream.next().await {
            // if let skystreamer::types::commit::Record::Post(post) = post {
            if let Err(e) = self.exporter.export(&post).await {
                tracing::error!("Failed to export post: {}", e);
            }
            self.update_stats();
        }

        Ok(())
    }

    pub fn update_stats(&mut self) {
        self.rate_counter.update();
        if self.rate_counter.rate_age_cycles() == 0 {
            tracing::info!(
                "Ingest rate: {rate:.2} items/s",
                rate = self.rate_counter.rate()
            );
        }
    }
}

// // #[derive(Debug)]
// pub struct FirehoseConsumer {
//     rate_counter: update_rate::DiscreteRateCounter,
//     // post_count: u64,
//     // start_time: std::time::Instant,
//     // exporter: dyn exporter::Exporter,
//     exporter: Box<dyn exporter::Exporter>,
// }

// impl FirehoseConsumer {
//     fn new(exporter: Box<dyn exporter::Exporter>) -> Self {
//         FirehoseConsumer {
//             rate_counter: update_rate::DiscreteRateCounter::new(50),
//             // post_count: 0,
//             // start_time: std::time::Instant::now(),
//             exporter,
//         }
//     }
// }

// impl CommitHandler for FirehoseConsumer {
//     async fn update_cursor(&self, seq: u64) -> Result<()> {
//         // no-op for now, I need to find a way to update the websocket params
//         tracing::trace!("Updating cursor to {}", seq);
//         // Implement the logic to update the cursor here
//         Ok(())
//     }
//     #[tracing::instrument(skip(self, commit))]
//     async fn handle_commit(&mut self, commit: &Commit) -> Result<()> {
//         for op in &commit.ops {
//             if !self.is_post_creation(op) {
//                 continue;
//             }

//             let record = self.extract_post_record(op, &commit.blocks).await?;
//             // get repo

//             let post = PostData::new(commit.repo.clone(), commit.commit.clone(), record);

//             let post = db_types::Post::new(post);

//             tracing::trace!(?post, "Received post");

//             self.exporter.export(&post).await?;

//             // let jsonl = serde_json::to_string(&post)?;

//             // // append to file called "posts.jsonl"

//             // let mut file = tokio::fs::OpenOptions::new()
//             //     .append(true)
//             //     .create(true)
//             //     .open("posts.jsonl")
//             //     .await?;

//             // tokio::io::AsyncWriteExt::write_all(&mut file, format!("{}\n", jsonl).as_bytes()).await?;

//             self.update_stats();
//         }
//         Ok(())
//     }
// }

// impl FirehoseConsumer {
//     fn is_post_creation(
//         &self,
//         op: &atrium_api::com::atproto::sync::subscribe_repos::RepoOp,
//     ) -> bool {
//         matches!(op.action.as_str(), "create") && op.path.split('/').next() == Some(BPost::NSID)
//     }

//     async fn extract_post_record(
//         &self,
//         op: &atrium_api::com::atproto::sync::subscribe_repos::RepoOp,
//         mut blocks: &[u8],
//     ) -> Result<Record> {
//         let (items, _) = rs_car::car_read_all(&mut blocks, true).await?;

//         let (_, item) = items
//             .iter()
//             .find(|(cid, _)| {
//                 let converted_cid = CidLink(
//                     types::CidOld::from(*cid)
//                         .try_into()
//                         .expect("invalid CID conversion"),
//                 );
//                 Some(converted_cid) == op.cid
//             })
//             .ok_or_else(|| {
//                 eyre!(
//                     "Could not find item with operation cid {:?} out of {} items",
//                     op.cid,
//                     items.len()
//                 )
//             })?;

//         Ok(serde_ipld_dagcbor::from_reader(&mut item.as_slice())?)
//     }
//     // #[tracing::instrument(skip(self))]
//     fn update_stats(&mut self) {
//         self.rate_counter.update();
//         if self.rate_counter.rate_age_cycles() == 0 {
//             tracing::info!(
//                 "Ingest rate: {rate:.2} items/s",
//                 rate = self.rate_counter.rate()
//             );
//         }
//     }
// }

fn default_level_filter() -> LevelFilter {
    #[cfg(debug_assertions)]
    return LevelFilter::DEBUG;
    #[cfg(not(debug_assertions))]
    return LevelFilter::INFO;
}

// use skystreamer::RepoSubscription;
// #![feature(cli)]
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // let default_levelfilter =

    let env_filter = EnvFilter::builder()
        .with_default_directive(default_level_filter().into())
        .from_env()?;

    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .with_file(false)
        .compact()
        .with_line_number(false)
        .with_env_filter(env_filter)
        .init();

    // start consumer
    // let main_task = tokio::spawn(async move {
    //     let config = crate::config::Config::parse();
    //     println!("{:?}", config);
    //     let mut consumer = config.consumer().await?;
    //     consumer.start().await?;
    //     Ok::<(), color_eyre::Report>(())
    // });
    // config.consumer().await?.start().await?;

    let config = crate::config::Config::parse();
    let mut consumer = config.consumer().await?;

    // ctrlc::set_handler(move || {
    //     LOCAL_THREAD_POOL.with(|pool| {
    //         pool.borrow_mut().handle_interrupt();
    //     });
    //     std::process::exit(0);
    // })
    // .expect("Error setting Ctrl-C handler");

    consumer.start().await?;

    Ok(())
}
