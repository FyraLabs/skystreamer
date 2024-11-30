//! Data export module for the bsky streamer.
//!
//! This module provides types, enums and functions for exporting data from the firehose.
//!
//!

pub const POSTS_TABLE: &str = "post";
pub const USERS_TABLE: &str = "user";

use std::{pin::Pin, sync::Arc};

use crate::{
    db_types::{Post as DbPost, User},
    types::Post,
};
use atrium_api::{
    agent::{store::MemorySessionStore, AtpAgent},
    app::bsky::actor::defs::ProfileViewDetailedData,
    types::{string::Did, Collection as _},
};
use atrium_xrpc_client::reqwest::ReqwestClient;
use chrono::NaiveDateTime;
use color_eyre::{eyre::OptionExt, Result};
use futures::StreamExt;
use std::sync::OnceLock;
use surrealdb::{Connection, RecordId, Surreal};
use tokio::io::AsyncWriteExt;

pub struct XrpcQuerier {
    pub client: Arc<AtpAgent<MemorySessionStore, ReqwestClient>>,
    pub http_client: reqwest::Client,
    pub semaphore: tokio::sync::Semaphore,
}

impl XrpcQuerier {
    fn new() -> Self {
        let agent = Arc::new(AtpAgent::new(
            ReqwestClient::new("https://public.api.bsky.app"),
            MemorySessionStore::default(),
        ));

        let http_client = reqwest::Client::new();

        let semaphore = tokio::sync::Semaphore::new(4);

        XrpcQuerier {
            client: agent,
            http_client,
            semaphore,
        }
    }

    pub fn get() -> &'static Self {
        QUERIER.get_or_init(XrpcQuerier::new)
    }

    pub async fn get_profile(&self, did: Did) -> Result<User> {
        let permit = self.semaphore.acquire().await?;

        let did_str = did.to_string();
        let now = chrono::Utc::now().naive_utc();

        // Check if profile is in cache first and not expired
        let cache = PROFILE_CACHE.with(|pc| pc.cache.clone());
        let mut cache = cache.write().await;

        if let Some((cached_time, profile)) = cache.get(&did_str).map(|(t, p)| (*t, p.clone())) {
            // 4 hour expiration time
            if now.signed_duration_since(cached_time).num_hours() < 4 {
                tracing::trace!(?profile, "Profile cache hit!");
                drop(permit);
                return Ok(profile);
            }
            // Remove expired entry
            cache.remove(&did_str);
        }
        drop(cache);

        let actor: User = self
            .http_client
            .get("https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile")
            .query(&[("actor", did_str.clone())])
            .send()
            .await?
            .json::<ProfileViewDetailedData>()
            .await?
            .into();

        let cache = PROFILE_CACHE.with(|pc| pc.cache.clone());
        let mut cache = cache.write().await;
        cache.insert(did_str, (now, actor.clone()));

        drop(permit);

        Ok(actor)
    }
}

pub struct ProfileCache {
    pub cache: Arc<tokio::sync::RwLock<std::collections::HashMap<String, (NaiveDateTime, User)>>>,
}

impl Default for ProfileCache {
    fn default() -> Self {
        ProfileCache {
            cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl ProfileCache {
    pub fn new() -> Self {
        Self::default()
    }
}
static QUERIER: OnceLock<XrpcQuerier> = OnceLock::new();

thread_local! {
    static PROFILE_CACHE: ProfileCache = ProfileCache::new();
}

#[async_trait::async_trait]
pub trait Exporter {
    async fn export(&mut self, post: &DbPost) -> Result<()>;
}

pub struct SurrealDbExporter<C: Connection> {
    db: Box<Surreal<C>>,
}

impl<C: Connection> SurrealDbExporter<C> {
    pub fn new(db: Surreal<C>) -> Self {
        SurrealDbExporter { db: Box::new(db) }
    }
}

async fn create_relations<C: Connection>(db: &Surreal<C>, post: &DbPost) -> Result<()> {
    // Spawn a background task to fetch and store the user profile
    let db = db.clone();
    let post_author = post.author.clone();
    let post_did = post.author_did.clone();
    let post_cid = post.cid.clone();
    tokio::spawn(async move {
        // let post_cid = post.cid.clone();
        // let permit = PROFILE_SEMAPHORE.with(|s| s.acquire()).await.unwrap();
        if let Err(e) = async {
            let actor = XrpcQuerier::get()
                .get_profile(post_did.ok_or_eyre("Post author DID is missing")?)
                .await?;

            let _: Option<User> = db
                .upsert((USERS_TABLE, post_author.key().to_owned()))
                .content(actor)
                .await?;

            let _ = db
                .query("BEGIN")
                .query("RELATE $user->author->$post")
                .bind(("user", post_author))
                .bind(("post", RecordId::from_table_key(POSTS_TABLE, post_cid)))
                .query("COMMIT")
                .await?;
            Ok::<_, color_eyre::Report>(())
        }
        .await
        {
            tracing::error!("Failed to fetch and store user profile: {}", e);
        }
        // drop(permit); // Explicitly drop the permit when done
    });
    Ok(())
}

#[async_trait::async_trait]
impl<C: Connection> Exporter for SurrealDbExporter<C> {
    async fn export(&mut self, post: &DbPost) -> Result<()> {
        // tracing::info!("Exporting post: {:?}", post);
        let res: Result<Option<DbPost>> = self
            .db
            .upsert((POSTS_TABLE, &post.cid))
            .content({
                // Strip out some unneeded fields for the post
                // let mut post_clone = post.clone();
                // post_clone.cid = None;
                // post_clone

                // post_clone.author = format!("r'user:{}'", &post.author);

                post.clone()
            })
            .await
            .map_err(|e| color_eyre::Report::new(e));

        if let Err(e) = res {
            tracing::error!(?post, "Failed to export post: {:?}", e);
        } else {
            // tracing::info!("Exported post: {:?}", a);

            create_relations(&self.db, post).await?;
        }

        // Right now the

        // if let Err(e) = a {
        //     tracing::error!("Failed to export post: {:?}", e);
        // } else {
        //     tracing::info!("Exported post: {:?}", a);
        // }

        Ok(())
    }
}

// export into jsonl, with Writer?

pub struct JsonlExporter<W: tokio::io::AsyncWrite + Unpin> {
    writer: W,
}

impl<W: tokio::io::AsyncWrite + Unpin> JsonlExporter<W> {
    pub fn new(writer: W) -> Self {
        JsonlExporter { writer }
    }
}

#[async_trait::async_trait]
impl<W: tokio::io::AsyncWrite + Unpin + Send> Exporter for JsonlExporter<W> {
    async fn export(&mut self, post: &DbPost) -> Result<()> {
        let json = serde_json::to_string(&post)?;
        self.writer
            .write_all(format!("{}\n", json).as_bytes())
            .await?;
        Ok(())
    }
}

pub struct CsvExporter<W: tokio::io::AsyncWrite + Unpin> {
    writer: W,
}

impl<W: tokio::io::AsyncWrite + Unpin> CsvExporter<W> {
    pub fn new(writer: W) -> Self {
        CsvExporter { writer }
    }
}

fn csv_escape(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('"', "\"\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace(',', "\\,")
    )
}

#[async_trait::async_trait]
impl<W: tokio::io::AsyncWrite + Unpin + Send> Exporter for CsvExporter<W> {
    async fn export(&mut self, post: &DbPost) -> Result<()> {
        let mut output = String::new();

        // Add fields with proper CSV escaping and comma delimiters
        output.push_str(&format!("{},", csv_escape(&post.cid)));
        output.push_str(&format!("{},", csv_escape(&post.author.key().to_string())));
        output.push_str(&format!("{},", csv_escape(&post.created_at.to_string())));
        output.push_str(&format!("{},", csv_escape(&post.language.join(";"))));
        output.push_str(&format!("{},", csv_escape(&post.text)));
        output.push_str(&format!(
            "{},",
            csv_escape(
                &post
                    .reply
                    .as_ref()
                    .map_or(String::new(), |reply| reply.reply_parent.to_string())
            )
        ));
        output.push_str(&format!(
            "{},",
            csv_escape(
                &post
                    .reply
                    .as_ref()
                    .map_or(String::new(), |reply| reply.reply_root.to_string())
            )
        ));

        // Join labels with semicolons instead of commas to avoid CSV confusion
        let labels = post
            .labels
            .iter()
            .map(|l| csv_escape(l))
            .collect::<Vec<_>>()
            .join(";");
        output.push_str(&format!("{},", labels));

        // Join tags with semicolons instead of commas to avoid CSV confusion
        let tags = post
            .tags
            .iter()
            .map(|t| csv_escape(t))
            .collect::<Vec<_>>()
            .join(";");
        output.push_str(&tags);

        self.writer.write_all(output.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;

        Ok(())
    }
}

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