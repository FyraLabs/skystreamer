//! Data export module for the bsky streamer.
//!
//! This module provides types, enums and functions for exporting data from the firehose.
//!
//!

pub const POSTS_TABLE: &str = "post";
pub const USERS_TABLE: &str = "user";

use std::sync::Arc;

use crate::db_types::{Post, User};
use atrium_api::{
    agent::{
        store::{MemorySessionStore, SessionStore},
        AtpAgent,
    },
    app::{
        self,
        bsky::actor::defs::{ProfileViewDetailed, ProfileViewDetailedData},
    },
    com,
    types::string::{AtIdentifier, Did},
    xrpc::{http::request, XrpcClient},
};
use atrium_xrpc_client::reqwest::ReqwestClient;
use chrono::NaiveDateTime;
use color_eyre::{eyre::OptionExt, Result};
use std::sync::OnceLock;
use surrealdb::{Connection, Surreal};
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

impl ProfileCache {
    pub fn new() -> Self {
        ProfileCache {
            cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}
static QUERIER: OnceLock<XrpcQuerier> = OnceLock::new();

thread_local! {
    static PROFILE_CACHE: ProfileCache = ProfileCache::new();
}

#[async_trait::async_trait]
pub trait Exporter {
    async fn export(&mut self, post: &Post) -> Result<()>;
}

pub struct SurrealDbExporter<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> SurrealDbExporter<C> {
    pub fn new(db: Surreal<C>) -> Self {
        SurrealDbExporter { db }
    }
}

async fn create_relations<C: Connection>(db: &Surreal<C>, post: &Post) -> Result<()> {
    // Spawn a background task to fetch and store the user profile
    let db = db.clone();
    let post_author = post.author.clone();
    let post_did = post.author_did.clone();
    tokio::spawn(async move {
        // let permit = PROFILE_SEMAPHORE.with(|s| s.acquire()).await.unwrap();
        if let Err(e) = async {
            let actor = XrpcQuerier::get()
                .get_profile(post_did.ok_or_eyre("Post author DID is missing")?)
                .await?;

            let _: Option<User> = db
                .upsert((USERS_TABLE, &post_author))
                .content(actor)
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
    async fn export(&mut self, post: &Post) -> Result<()> {
        // tracing::info!("Exporting post: {:?}", post);
        let res: Result<Option<Post>> = self
            .db
            .upsert((POSTS_TABLE, &post.cid))
            .content({
                // Strip out some unneeded fields for the post
                // let mut post_clone = post.clone();
                // post_clone.cid = None;
                // post_clone

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
    async fn export(&mut self, post: &Post) -> Result<()> {
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

#[async_trait::async_trait]
impl<W: tokio::io::AsyncWrite + Unpin + Send> Exporter for CsvExporter<W> {
    async fn export(&mut self, post: &Post) -> Result<()> {
        let mut csv = csv::StringRecord::new();
        csv.push_field(post.cid.as_str());
        csv.push_field(post.author.as_str());
        csv.push_field(post.created_at.to_string().as_str());
        csv.push_field(post.language.as_str());
        csv.push_field(post.text.as_str());
        csv.push_field(
            post.reply
                .as_ref()
                .map_or("", |reply| reply.reply_parent.as_str()),
        );
        csv.push_field(
            post.reply
                .as_ref()
                .map_or("", |reply| reply.reply_root.as_str()),
        );

        let labels = post
            .labels
            .iter()
            .fold(String::new(), |acc, label| acc + label + ",");

        csv.push_field(labels.as_str());

        let tags = post
            .tags
            .iter()
            .fold(String::new(), |acc, tag| acc + tag + ",");

        csv.push_field(tags.as_str());

        // don't include embeds

        self.writer.write_all(csv.as_slice().as_bytes()).await?;

        Ok(())
    }
}
