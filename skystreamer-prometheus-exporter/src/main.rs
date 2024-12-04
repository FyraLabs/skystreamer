use color_eyre::Result;
use futures::StreamExt;
use prometheus_exporter::{self, prometheus::register_int_counter};
use skystreamer::{stream::PostStream, RepoSubscription};
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
        .unwrap_or_else(|_| "10000".to_string())
        .parse::<usize>()?;

    tracing::info!("Starting skystreamer-prometheus-exporter");
    tracing::info!("MAX_SAMPLE_SIZE: {}", max_sample_size);

    let binding = "0.0.0.0:9100".parse()?;
    let _exporter = prometheus_exporter::start(binding)?;
    let counter = register_int_counter!(
        "skystreamer_bsky_posts",
        "Number of posts from bsky.network"
    )?;

    let language_counter = prometheus_exporter::prometheus::register_int_counter_vec!(
        "skystreamer_bsky_posts_by_language",
        "Number of posts from bsky.network by language",
        &["language"]
    )?;

    // const MAX_SAMPLE_SIZE: usize = 10000;

    loop {
        let subscription = RepoSubscription::new("bsky.network").await.unwrap();
        let post_stream = PostStream::new(subscription);
        let mut post_stream = post_stream.await;
        let stream = post_stream.stream().await?;

        futures::pin_mut!(stream);
        // let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        // interval.tick().await;

        // let mut last_tick = tokio::time::Instant::now();

        while let Some(post) = stream.next().await {
            counter.inc();

            let langs = post.language.join(",");
            language_counter.with_label_values(&[&langs]).inc();

            if counter.get() > max_sample_size as u64 {
                counter.reset();
            }
        }
    }

    // Ok(())
}
