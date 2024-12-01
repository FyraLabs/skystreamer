use atrium_api::app::bsky::actor::get_profile;
use atrium_api::{
    agent::{store::MemorySessionStore, AtpAgent},
    app::bsky::actor::defs::ProfileViewDetailedData,
    types::string::{AtIdentifier, Did},
};
use atrium_xrpc_client::reqwest::ReqwestClient;
use chrono::NaiveDateTime;
use color_eyre::{eyre::OptionExt, Result};
use ipld_core::ipld::Ipld;
use skystreamer::types::Post as SPost;
use std::{
    str::FromStr,
    sync::{Arc, OnceLock},
};
use surrealdb::{Connection, RecordId, Surreal};
use tokio::io::AsyncWriteExt;
pub const POSTS_TABLE: &str = "post";
pub const USERS_TABLE: &str = "user";
use crate::surreal_types::{SurrealPostRep, User};
use crate::LOCAL_THREAD_POOL;

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

    pub async fn get_profile_xrpc(&self, did: Did) -> Result<User> {
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

        let actor = self
            .client
            .api
            .app
            .bsky
            .actor
            .get_profile(get_profile::Parameters {
                data: get_profile::ParametersData {
                    actor: AtIdentifier::Did(did),
                },
                extra_data: Ipld::Null,
            })
            .await?
            .data;

        let actor: User = actor.into();

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

pub struct DryRunExporter;

#[async_trait::async_trait]
impl Exporter for DryRunExporter {
    async fn export(&mut self, post: &SPost) -> Result<()> {
        tracing::info!("Dry run: {:?}", post);
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait Exporter {
    async fn export(&mut self, post: &SPost) -> Result<()>;
}

pub struct SurrealDbExporter<C: Connection> {
    db: Box<Surreal<C>>,
}

impl<C: Connection> SurrealDbExporter<C> {
    pub fn new(db: Surreal<C>) -> Self {
        SurrealDbExporter { db: Box::new(db) }
    }
}

async fn create_relations<C: Connection>(
    db: &Surreal<C>,
    post: &crate::surreal_types::SurrealPostRep,
) -> Result<()> {
    // Spawn a background task to fetch and store the user profile
    let db = db.clone();
    let post_author = post.author.key().to_string();
    let post_did = Did::from_str(&format!("did:plc:{}", post.author.key()))
        .map_err(|e| color_eyre::eyre::eyre!("Invalid DID: {}", e))?;
    let post_cid = post
        .id
        .clone()
        .ok_or_else(|| color_eyre::eyre::eyre!("Post has no ID"))?
        .id
        .to_string();
    let actor = XrpcQuerier::get().get_profile_xrpc(post_did).await.ok();

    // tracing::info!("Creating relations for post: {}", post_cid);
    if let Err(e) = async {
        let _: Option<User> = db
            .upsert((USERS_TABLE, &post_author))
            .content(actor)
            .await?;

        db.query("RELATE $user->author->$post")
            .bind(("user", post_author))
            .bind(("post", RecordId::from_table_key(POSTS_TABLE, post_cid)))
            .await?;

        Ok::<_, color_eyre::Report>(())
    }
    .await
    {
        tracing::error!("Failed to create relations: {:?}", e);
    }

    // LOCAL_THREAD_POOL.with(|tp| {
    //     tp.borrow_mut().add_task(task);
    //     // tracing::info!("Workers: {}", tp.borrow().workers.len());
    //     // tp.borrow().workers.len()
    // });

    Ok(())
}

#[async_trait::async_trait]
impl<C: Connection> Exporter for SurrealDbExporter<C> {
    async fn export(&mut self, post: &SPost) -> Result<()> {
        let db = self.db.clone();
        let post = post.clone();
        tokio::spawn(async move {
            let post_rep: crate::surreal_types::SurrealPostRep = post.clone().into();
            let res: Option<crate::surreal_types::SurrealPostRep> = db
                .upsert((POSTS_TABLE, &post.id.to_string()))
                .content(post_rep)
                .await
                .map_err(|e| color_eyre::Report::new(e))?;

            create_relations(
                &db,
                &res.ok_or_else(|| color_eyre::eyre::eyre!("Post not found!"))?,
            )
            .await?;
            Ok::<_, color_eyre::Report>(())
        });
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
    async fn export(&mut self, post: &SPost) -> Result<()> {
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
    s.replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        // .replace(',', "\\,")
        .to_string()
}

#[async_trait::async_trait]
impl<W: tokio::io::AsyncWrite + Unpin + Send> Exporter for CsvExporter<W> {
    async fn export(&mut self, post: &SPost) -> Result<()> {
        let mut writer = csv_async::AsyncWriter::from_writer(&mut self.writer);

        // writer
        //     .write_record(&[
        //         "ID",
        //         "Author",
        //         "Content",
        //         "Created At",
        //         "Text",
        //         "Labels",
        //         "Tags",
        //     ])
        //     .await?;
        // note: this may look weird, but trust me this will output a completely valid RFC 4180 CSV
        // the linebreaks will look weird and enclosed in quotes
        writer
            .write_record(&[
                post.id.to_string(),
                post.author.to_string(),
                post.text.to_string(),
                post.created_at.to_string(),
                post.text.to_string(),
                post.labels.join(";"),
                post.tags.join(";"),
            ])
            .await?;

        writer.flush().await?;

        Ok(())
    }
}
