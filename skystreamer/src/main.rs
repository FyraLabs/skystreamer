// use anyhow::{anyhow, Result};
// use crate::types::Frame;
use atrium_api::{
    app::bsky::feed::{post::Record, Post as BPost},
    com::atproto::sync::subscribe_repos::{Commit, NSID},
    types::{CidLink, Collection},
};
use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use futures::StreamExt;
use skystreamer::RepoSubscription;
use surrealdb::{engine::remote::ws::Ws, Surreal};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .with_file(false)
        .compact()
        .with_line_number(false)
        .with_env_filter("info")
        .init();

    let config = skystreamer::config::Config::parse();

    println!("{:?}", config);
    // let db = Surreal::new::<Ws>("localhost:8000").await?;

    let consumer = config.subscribe().await?;

    // db.use_ns("bsky.network").use_db("bsky").await?;
    // // Load schema from file
    // let schema = include_str!("schema.surql");

    // tracing::info!("Loading schema");
    // db.query(schema).await?;

    // let surreal_exporter = exporter::SurrealDbExporter::new(db);

    // let consumer = FirehoseConsumer::new(Box::new(surreal_exporter));

    // RepoSubscription::new("bsky.network")
    //     .await
    //     .unwrap()
    //     .run(consumer)
    //     .await
    //     .unwrap();

    RepoSubscription::new("bsky.network")
        .await?
        .run(consumer)
        .await?;

    Ok(())
}
