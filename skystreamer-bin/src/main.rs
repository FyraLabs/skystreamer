// use anyhow::{anyhow, Result};
// use crate::types::Frame;
mod config;
mod exporter;
mod surreal_types;
use std::sync::Arc;

use futures::StreamExt;
pub struct Consumer {
    rate_counter: update_rate::DiscreteRateCounter,
    exporter: Box<dyn exporter::Exporter>,
}
#[derive(Debug)]
pub struct TaskQueue {
    workers: Vec<tokio::task::JoinHandle<()>>,
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl TaskQueue {
    pub fn new() -> Self {
        TaskQueue {
            workers: Vec::new(),
            semaphore: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }

    /// Add a new task on the queue from a tokio join handle
    /// Remove the task from the queue when it finishes
    pub fn add_task(&mut self, task: tokio::task::JoinHandle<()>) {
        self.workers.retain(|worker| !worker.is_finished());
        // tracing::info!("Running workers: {}", self.workers.len());
        let semaphore = self.semaphore.clone();
        let worker = tokio::spawn(async move {
            let _permit = semaphore.acquire().await;
            tracing::info!("Available permits: {}", semaphore.available_permits());
            tokio::join!(task).0.unwrap();
            // release permit when task is done
        });
        self.workers.push(worker);
    }

    pub fn handle_interrupt(&mut self) {
        for worker in self.workers.drain(..) {
            self.semaphore.clone().close();
            tracing::info!("Cancelling workers");
            worker.abort();
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static LOCAL_THREAD_POOL: std::cell::RefCell<TaskQueue> = std::cell::RefCell::new(TaskQueue::new());
}

// const GLOBAL_THREAD_POOL: OnceCell<ThreadPool> = OnceCell::new();

impl Consumer {
    pub fn new(exporter: Box<dyn exporter::Exporter>) -> Self {
        Consumer {
            rate_counter: update_rate::DiscreteRateCounter::new(50),
            exporter,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let subscription = RepoSubscription::new("bsky.network").await.unwrap();
        let post_stream = PostStream::new(subscription);

        let mut post_stream = post_stream.await;
        let stream = post_stream.stream().await?;

        futures::pin_mut!(stream);

        while let Some(post) = stream.next().await {
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
use clap::Parser;
use color_eyre::Result;
use skystreamer::{stream::PostStream, RepoSubscription};

use update_rate::RateCounter;
// use skystreamer::RepoSubscription;
// #![feature(cli)]
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .with_file(false)
        .compact()
        .with_line_number(false)
        .with_env_filter("info")
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

    ctrlc::set_handler(move || {
        LOCAL_THREAD_POOL.with(|pool| {
            pool.borrow_mut().handle_interrupt();
        });
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    consumer.start().await?;

    Ok(())
}
