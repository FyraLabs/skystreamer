//! Data export module for the bsky streamer.
//!
//! This module provides types, enums and functions for exporting data from the firehose.
//!
//!

pub const POSTS_TABLE: &str = "post";
pub const USERS_TABLE: &str = "user";

use crate::db_types::Post;
use color_eyre::Result;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use surrealdb::{Connection, Object, Surreal};
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;

#[async_trait::async_trait]
pub trait Exporter {
    async fn export(&mut self, post: Post) -> Result<()>;
}

pub struct SurrealDbExporter<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> SurrealDbExporter<C> {
    pub fn new(db: Surreal<C>) -> Self {
        SurrealDbExporter { db }
    }
}

#[async_trait::async_trait]
impl<C: Connection> Exporter for SurrealDbExporter<C> {
    async fn export(&mut self, post: Post) -> Result<()> {
        // tracing::info!("Exporting post: {:?}", post);
        let a: Result<Option<Post>> = self
            .db
            .create((POSTS_TABLE, &post.id.to_string()))
            .content(post)
            .await
            .map_err(|e| color_eyre::Report::new(e));

        // Right now the err

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
    async fn export(&mut self, post: Post) -> Result<()> {
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
    async fn export(&mut self, post: Post) -> Result<()> {
        let mut csv = csv::StringRecord::new();
        csv.push_field(post.id.as_str());
        csv.push_field(post.author.as_str());
        csv.push_field(post.created_at.to_string().as_str());
        csv.push_field(post.language.as_str());
        csv.push_field(post.text.as_str());
        csv.push_field(
            post.reply
                .as_ref()
                .map_or("", |reply| reply.parent.as_str()),
        );
        csv.push_field(post.reply.as_ref().map_or("", |reply| reply.root.as_str()));

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
