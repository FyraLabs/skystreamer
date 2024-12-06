use color_eyre::Result;
use prometheus_exporter::prometheus::register_int_counter_vec;
use prometheus_exporter::{self, prometheus::register_int_counter};
// use skystreamer::{stream::EventStream, types::commit::Record, RepoSubscription};
use url::Url;

use crate::util::handle_language;

pub struct PostsRegistry {
    normalize_langs: bool,
    post_counter: prometheus_exporter::prometheus::IntCounter,
    language_counter: prometheus_exporter::prometheus::IntCounterVec,
    language_counter_individual: prometheus_exporter::prometheus::IntCounterVec,
    label_counter: prometheus_exporter::prometheus::IntCounterVec,
    tags_counter: prometheus_exporter::prometheus::IntCounterVec,
    quote_counter: prometheus_exporter::prometheus::IntCounterVec,
    reply_counter: prometheus_exporter::prometheus::IntCounterVec,
    alt_text_counter: prometheus_exporter::prometheus::IntCounterVec,
    media_posts_counter: prometheus_exporter::prometheus::IntCounterVec,
    posts_external_links: prometheus_exporter::prometheus::IntCounterVec,

    // domain_reset_time: tokio::time::Instant,

    // reset the counter
    // and repopulate it with the current domains whose last updated time is less than 30 minutes ago
    // the oldest domains will be removed first
    // then the new domain will be added
    domains: std::collections::HashMap<String, (tokio::time::Instant, u64)>,
    tags: std::collections::HashMap<String, (tokio::time::Instant, u64)>,
    labels: std::collections::HashMap<String, (tokio::time::Instant, u64)>,
}

impl PostsRegistry {
    // pub fn reset_timer(&mut self) {
    //     self.domain_reset_time =
    //         tokio::time::Instant::now() + tokio::time::Duration::from_secs(1800);
    // }

    pub fn update_tag(&mut self, tag: &str) {
        let tag_entry = self
            .tags
            .entry(tag.to_string())
            .or_insert_with(|| (tokio::time::Instant::now(), 0));

        tag_entry.1 += 1;
        tag_entry.0 = tokio::time::Instant::now();

        self.tags_counter.reset();

        self.tags
            .retain(|_, (time, _)| time.elapsed() < tokio::time::Duration::from_secs(1800));

        for (tag, (_time, count)) in self.tags.iter() {
            self.tags_counter
                .with_label_values(&[tag])
                .inc_by((*count as i64).try_into().unwrap());
        }
    }

    pub fn update_domain(&mut self, domain: &str) {
        let domain_entry = self
            .domains
            .entry(domain.to_string())
            .or_insert_with(|| (tokio::time::Instant::now(), 0));

        domain_entry.1 += 1;
        domain_entry.0 = tokio::time::Instant::now();

        // tracing::info!("Domain count is larger than 10, truncating oldest domains");
        self.posts_external_links.reset();

        // domains whose last updated time is less than 30 minutes ago
        self.domains
            .retain(|_, (time, _)| time.elapsed() < tokio::time::Duration::from_secs(1800));
        // }

        for (domain, (_time, count)) in self.domains.iter() {
            self.posts_external_links
                .with_label_values(&[domain])
                .inc_by((*count as i64).try_into().unwrap());
        }
    }

    pub fn update_label(&mut self, label: &str) {
        let label_entry = self
            .labels
            .entry(label.to_string())
            .or_insert_with(|| (tokio::time::Instant::now(), 0));

        label_entry.1 += 1;
        label_entry.0 = tokio::time::Instant::now();

        self.label_counter.reset();

        self.labels
            .retain(|_, (time, _)| time.elapsed() < tokio::time::Duration::from_secs(1800));

        for (label, (_time, count)) in self.labels.iter() {
            self.label_counter
                .with_label_values(&[label])
                .inc_by((*count as i64).try_into().unwrap());
        }
    }

    pub fn new(normalize_langs: bool) -> Result<Self> {
        Ok(Self{
            // 30 minute reset
            // domain_reset_time: tokio::time::Instant::now() + tokio::time::Duration::from_secs(1800),
            domains: std::collections::HashMap::new(),
            tags: std::collections::HashMap::new(),
            labels: std::collections::HashMap::new(),
            normalize_langs,
            post_counter: register_int_counter!(
                "skystreamer_bsky_posts",
                "Number of posts from bsky.network"
            )?,
            language_counter: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_language_grouped",
                "Number of posts from bsky.network by language",
                &["language"]
            )?,
            language_counter_individual: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_language",
                "Number of posts from bsky.network by language (individually)",
                &["language"]
            )?,
            label_counter: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_label",
                "Number of posts from bsky.network by label",
                &["post_label"]
            )?,
            tags_counter: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_tag",
                "Number of posts from bsky.network by tag",
                &["post_tag"]
            )?,
            quote_counter: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_quote",
                "Number of quote posts from bsky.network",
                &["quote"]
            )?,
            reply_counter: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_reply",
                "Number of reply posts from bsky.network",
                &["reply"]
            )?,
            alt_text_counter: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_alt_text",
                "Number of posts from bsky.network by whether they have alt text",
                &["alt_text"]
            )?,
            media_posts_counter: register_int_counter_vec!(
                "skystreamer_bsky_posts_by_media",
                "Number of posts from bsky.network by whether they have media, labeled by type of media they have",
                &["media"]
            )?,
            posts_external_links: register_int_counter_vec!(
                "skystreamer_bsky_posts_external_links",
                "Number of posts from bsky.network that have external links by domain",
                &["domain"]
            )?,
        })
    }

    pub fn handle_post(&mut self, post: &skystreamer::types::Post) -> Result<()> {
        // Total number of posts
        self.post_counter.inc();
        {
            // Count the number of posts by label
            for label in post.labels.iter() {
                // self.label_counter.with_label_values(&[label]).inc();
                self.update_label(label);
            }

            // Count the number of posts by tag
            for tag in post.tags.iter() {
                self.update_tag(tag);
                // self.tags_counter.with_label_values(&[tag]).inc();
            }
        }

        // Quote Posts
        {
            let is_quote = if let Some(embed) = post.embed.as_ref() {
                matches!(
                    embed,
                    skystreamer::types::Embed::Record(_)
                        | skystreamer::types::Embed::RecordWithMedia(_, _)
                )
            } else {
                false
            };

            if is_quote {
                self.quote_counter.with_label_values(&["true"]).inc();
            }
        }
        // End Quote Posts

        let post_media = post.get_post_media();

        // Posts with media that have alt text
        {
            let has_alt_text = post_media.iter().any(|media| match media {
                skystreamer::types::Media::Image(i) => !i.alt.is_empty(),
                skystreamer::types::Media::Video(v) => v.alt.is_some(),
            });

            if has_alt_text {
                self.alt_text_counter.with_label_values(&["true"]).inc();
            };
        }
        {
            // Posts with media by type
            // let post_media = post.get_post_media(post);
            if !post_media.is_empty() {
                // get the first media type, because images and videos are mutually exclusive
                let media_type = match post_media.first().unwrap() {
                    skystreamer::types::Media::Image(_) => "image",
                    skystreamer::types::Media::Video(_) => "video",
                };
                self.media_posts_counter
                    .with_label_values(&[media_type])
                    .inc();
            }
        }
        {
            // Posts that are replies to other posts
            let is_reply = post.reply.is_some();

            if is_reply {
                self.reply_counter.with_label_values(&["reply"]).inc();
            }
        }

        // Posts by external media's domain name
        {
            let external_link = post.embed.clone().and_then(|embed| match embed {
                skystreamer::types::Embed::External(e) => Some(Url::parse(&e.uri)),
                _ => None,
            });

            // // Let's clean up some old domains to keep the response sizes down
            // if self.domain_reset_time < tokio::time::Instant::now() {
            //     self.posts_external_links.reset();
            //     self.reset_timer();
            // }

            // get domain
            let domain_name = external_link.and_then(|link| link.ok()).and_then(|url| {
                url.domain().map(|domain| {
                    domain
                        .strip_prefix("www.")
                        .unwrap_or(domain)
                        .to_string()
                        .to_lowercase()
                })
            });

            if let Some(domain) = domain_name {
                // update the domain counter
                self.update_domain(&domain);
            }
        }

        {
            // Languages of the posts
            let mut langs = post
                .language
                .iter()
                .map(|lang| {
                    // Let's normalize all the languages to its main language

                    let processed_language = if lang.is_empty() {
                        "null".to_string()
                    } else if self.normalize_langs {
                        let l = handle_language(lang);
                        if l.is_none() {
                            tracing::warn!("Failed to normalize language: {}", lang);
                        }
                        l.unwrap_or_else(|| lang.to_lowercase())
                    } else {
                        lang.to_string()
                    };

                    self.language_counter_individual
                        .with_label_values(&[&processed_language])
                        .inc();

                    processed_language
                })
                .collect::<Vec<_>>();

            // sort the languages
            langs.sort();

            langs.iter().for_each(|lang| {
                self.language_counter_individual
                    .with_label_values(&[lang])
                    .inc();
            });

            let langs_unique = langs
                .iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>();

            let langs_joined = if langs.is_empty() {
                "null".to_string()
            } else {
                langs_unique.into_iter().collect::<Vec<_>>().join(",")
            };
            self.language_counter
                .with_label_values(&[&langs_joined])
                .inc();

            Ok(())
        }
    }
}
