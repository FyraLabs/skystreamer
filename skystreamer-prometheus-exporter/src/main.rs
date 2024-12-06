mod posts;
mod util;
use color_eyre::Result;
use futures::StreamExt;
use posts::PostsRegistry;
use prometheus_exporter::{
    self,
    prometheus::{register_int_counter, register_int_counter_vec},
};
use skystreamer::{stream::EventStream, types::commit::Record, RepoSubscription};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

fn default_level_filter() -> LevelFilter {
    #[cfg(debug_assertions)]
    return LevelFilter::DEBUG;
    #[cfg(not(debug_assertions))]
    return LevelFilter::INFO;
}

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(default_level_filter().into())
        .from_env()?;

    color_eyre::install()?;

    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .with_file(false)
        .compact()
        .with_line_number(false)
        .with_env_filter(env_filter)
        .init();

    let max_sample_size = std::env::var("MAX_SAMPLE_SIZE")
        .map(|val| val.parse::<usize>().unwrap_or(10000))
        .ok();

    let normalize_langs = std::env::var("NORMALIZE_LANGS")
        .map(|val| val.parse::<bool>().unwrap_or(true))
        .ok();

    tracing::info!("Starting skystreamer-prometheus-exporter");
    tracing::info!("MAX_SAMPLE_SIZE: {:?}", max_sample_size);

    let binding = "0.0.0.0:9100".parse()?;
    let _exporter = prometheus_exporter::start(binding)?;

    let primary_counter = register_int_counter!(
        "skystreamer_atproto_events",
        "Total number of events from the AT Firehose"
    )?;

    let type_counter = register_int_counter_vec!(
        "skystreamer_atproto_event_typed",
        "Total number of events from the AT Firehose",
        &["type"]
    )?;

    // const MAX_SAMPLE_SIZE: usize = 10000;

    let subscription = RepoSubscription::new("bsky.network")
        .await
        .expect("Failed to create subscription");

    let mut event_stream = EventStream::new(subscription);

    let posts = std::sync::Arc::new(std::sync::Mutex::new(PostsRegistry::new(
        normalize_langs.unwrap_or(true),
    )?));

    let stream = event_stream.stream().await?;

    // let post_stream = PostStream::new(subscription);
    // let mut post_stream = post_stream.await;
    // let stream = post_stream.stream().await?;

    futures::pin_mut!(stream);
    // let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    // interval.tick().await;

    // let mut last_tick = tokio::time::Instant::now();

    while let Ok(Some(record)) =
        tokio::time::timeout(std::time::Duration::from_secs(10), stream.next()).await
    {
        primary_counter.inc();

        let posts_registry = posts.clone();
        let type_counter = type_counter.clone();
        tokio::spawn(async move {
            match record {
                Record::Post(post) => {
                    type_counter.with_label_values(&["post"]).inc();
                    tokio::spawn(async move {
                        let mut posts_registry = posts_registry.lock().unwrap();
                        posts_registry.handle_post(&post)?;
                        Ok::<(), color_eyre::eyre::Report>(())
                    });
                }
                Record::Block(_) => {
                    type_counter.with_label_values(&["block"]).inc();
                    // todo
                }
                Record::Like(_) => {
                    type_counter.with_label_values(&["like"]).inc();
                    // todo
                }
                Record::Follow(_) => {
                    type_counter.with_label_values(&["follow"]).inc();
                    // todo
                }
                Record::Repost(_) => {
                    type_counter.with_label_values(&["repost"]).inc();
                    // todo
                }
                Record::ListItem(_) => {
                    type_counter.with_label_values(&["list_item"]).inc();
                    // todo
                }
                Record::Profile(_) => {
                    type_counter.with_label_values(&["profile"]).inc();
                    // todo
                }
                _ => {
                    // todo
                }
            }

            Ok::<(), color_eyre::eyre::Report>(())
        });
    }
    tracing::error!("Stream ended");
    return Err(color_eyre::eyre::eyre!("Stream ended"));
    // Ok(())
}
