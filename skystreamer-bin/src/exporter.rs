use atrium_api::app::bsky::actor::get_profile;
use atrium_api::{
    agent::{store::MemorySessionStore, AtpAgent},
    types::string::{AtIdentifier, Did},
};
use atrium_xrpc_client::reqwest::ReqwestClient;
use chrono::NaiveDateTime;
use color_eyre::Result;
use ipld_core::ipld::Ipld;
use skystreamer::types::Post as SPost;
use std::sync::{Arc, OnceLock};
use surrealdb::{Connection, Surreal};
use tokio::io::AsyncWriteExt;
pub const POSTS_TABLE: &str = "post";
pub const USERS_TABLE: &str = "user";
use crate::config::will_fetch_user_data;
use crate::surreal_types::{Embed, User};

pub struct XrpcQuerier {
    pub client: Arc<AtpAgent<MemorySessionStore, ReqwestClient>>,
    // pub http_client: reqwest::Client,
    pub semaphore: tokio::sync::Semaphore,
}

impl XrpcQuerier {
    fn new() -> Self {
        let agent = Arc::new(AtpAgent::new(
            ReqwestClient::new("https://public.api.bsky.app"),
            MemorySessionStore::default(),
        ));

        // let http_client = reqwest::Client::new();

        let semaphore = tokio::sync::Semaphore::new(4);

        XrpcQuerier {
            client: agent,
            // http_client,
            semaphore,
        }
    }

    pub fn get() -> &'static Self {
        QUERIER.get_or_init(XrpcQuerier::new)
    }

    // pub async fn get_profile(&self, did: Did) -> Result<User> {
    //     let permit = self.semaphore.acquire().await?;

    //     let did_str = did.to_string();
    //     let now = chrono::Utc::now().naive_utc();

    //     // Check if profile is in cache first and not expired
    //     let cache = PROFILE_CACHE.with(|pc| pc.cache.clone());
    //     let mut cache = cache.write().await;

    //     if let Some((cached_time, profile)) = cache.get(&did_str).map(|(t, p)| (*t, p.clone())) {
    //         // 4 hour expiration time
    //         if now.signed_duration_since(cached_time).num_hours() < 4 {
    //             tracing::trace!(?profile, "Profile cache hit!");
    //             drop(permit);
    //             return Ok(profile);
    //         }
    //         // Remove expired entry
    //         cache.remove(&did_str);
    //     }
    //     drop(cache);

    //     let actor: User = self
    //         .http_client
    //         .get("https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile")
    //         .query(&[("actor", did_str.clone())])
    //         .send()
    //         .await?
    //         .json::<ProfileViewDetailedData>()
    //         .await?
    //         .into();

    //     let cache = PROFILE_CACHE.with(|pc| pc.cache.clone());
    //     let mut cache = cache.write().await;
    //     cache.insert(did_str, (now, actor.clone()));

    //     drop(permit);

    //     Ok(actor)
    // }

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
                tracing::debug!(?profile, "Profile cache hit!");
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
        tracing::debug_span!("profile_cache_insert").in_scope(|| {
            tracing::debug!(?actor, "Inserting into cache");
            cache.insert(did_str.clone(), (now, actor.clone()));
        });
        // cache.insert(did_str, (now, actor.clone()));

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
    db: Arc<tokio::sync::Mutex<Surreal<C>>>,
}

impl<C: Connection> SurrealDbExporter<C> {
    pub fn new(db: Surreal<C>) -> Self {
        SurrealDbExporter {
            db: Arc::new(tokio::sync::Mutex::new(db)),
        }
    }
}

async fn create_relations<C: Connection>(
    db: &Surreal<C>,
    post: &crate::surreal_types::SurrealPostRep,
) -> Result<()> {
    // Spawn a background task to fetch and store the user profile
    let post_author = post.author.key().to_string();
    let post_did = post.get_author_did()?;
    // let post_cid = post
    //     .id
    //     .clone()
    //     .ok_or_else(|| color_eyre::eyre::eyre!("Post has no ID"))?
    //     .id
    //     .to_string();

    // Create dummy table entry for user if it doesn't exist
    tracing::debug_span!("create_empty_user_profile", ?post_author).in_scope(|| {
        let db_clone = db.clone();
        let post_author_clone = post_author.clone();
        tokio::spawn(async move {
            if let Err(e) = async {
                let _: Option<User> = db_clone
                    .upsert((USERS_TABLE, &post_author_clone))
                    .content(User::default())
                    .await?;
                Ok::<_, color_eyre::Report>(())
            }
            .await
            {
                tracing::error!("Failed to create empty user profile: {:?}", e);
            }
        });
    });
    // Fetch user profile if needed
    tracing::debug_span!("user_fetch", ?post_did).in_scope(|| {
        if will_fetch_user_data() {
            let db = db.clone();
            tokio::spawn(async move {
                let actor = XrpcQuerier::get().get_profile(post_did).await.ok();
                if let Err(e) = async {
                    let _: Option<User> = db
                        .upsert((USERS_TABLE, &post_author))
                        .content(actor.clone().unwrap_or_default())
                        .await?;
                    Ok::<_, color_eyre::Report>(())
                }
                .await
                {
                    tracing::error!("Failed to create user profile: {:?}", e);
                }
            });
        }
    });

    if let Err(e) = async {
        let post = post.clone();
        let db = db.clone();

        let mut query = db.query("BEGIN;").query("RELATE $user->author->$post");

        if let Some(reply_ref) = &post.reply {
            query = query
                .query("RELATE $post->reply_parent->$reply_parent")
                .query("RELATE $post->reply_root->$reply_root")
                .bind(("reply_parent", reply_ref.reply_parent.clone()))
                .bind(("reply_root", reply_ref.reply_root.clone()));
        }

        if let Some(embed) = &post.embed {
            let quote_id = match embed {
                Embed::Record(r) => Some(r),
                Embed::RecordWithMedia { record, media: _ } => Some(record),
                _ => None,
            };

            if let Some(quote_id) = quote_id {
                query = query
                    .query("RELATE $post->quoted->$quote_id")
                    .bind(("quote_id", quote_id.clone()));
            }
        }

        let res = query
            .bind(("user", post.author))
            .bind(("post", post.id))
            .query("COMMIT;")
            .await?
            .check()?;

        tracing::debug!(?res, "Created author relation");
        // res.check()?;

        Ok::<_, color_eyre::Report>(())
    }
    .await
    {
        tracing::error!("Failed to create relations: {:?}", e);
    } else {
        tracing::debug!(?post, "Created relations for post");
    }

    Ok(())
}

#[async_trait::async_trait]
impl<C: Connection> Exporter for SurrealDbExporter<C> {
    async fn export(&mut self, post: &SPost) -> Result<()> {
        let db = self.db.clone();
        let post = post.clone();
        tokio::spawn(async move {
            // note: Locking the database here for some reason fixes
            // timing issues with the database
            let db = db.lock().await;
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

// fn csv_escape(s: &str) -> String {
//     s.replace('\n', "\\n")
//         .replace('\r', "\\r")
//         .replace('\t', "\\t")
//         // .replace(',', "\\,")
//         .to_string()
// }

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
