use clap::{Parser, ValueEnum};
use color_eyre::Result;
use surrealdb::{opt::auth::Root, Surreal};

use crate::Consumer;
// use crate::
#[derive(Debug, ValueEnum, Clone)]
pub enum SurrealAuthType {
    /// Use a token for authentication
    Token,
    /// Root-level authentication
    Root,
    /// Namespace authentication
    Namespace,
}

#[derive(Debug, ValueEnum, Clone, Default)]
pub enum ExporterType {
    /// Export to a JSONL (JSON Lines) file
    Jsonl,
    /// Export to a CSV file
    Csv,
    /// Export to a SurrealDB instance
    #[default]
    Surrealdb,

    /// Do not export anywhere, just log the data.
    /// This is useful for testing
    DryRun,
}

#[derive(Parser, Debug, Clone)]
pub struct FileExporterOptions {
    /// Path to the file to export to
    #[clap(
        short = 'o',
        long,
        required_if_eq("exporter", "jsonl"),
        required_if_eq("exporter", "csv"),
        env = "FILE_EXPORT_PATH",
        group = "file_exporter"
    )]
    pub file_path: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct SurrealDbConn {
    /// SurrealDB endpoint
    #[clap(
        short = 'e',
        long,
        // default_value_if("surreal_conn_type", "Websocket", "ws://localhost:8000"),
        // default_value_if("surreal_conn_type", "Http", "http://localhost:8000"),
        required_if_eq("exporter", "surrealdb"),
        default_value = "ws://localhost:8000",
        env = "SURREAL_ENDPOINT",
        // group = "surrealdb"
    )]
    pub surreal_endpoint: String,

    /// Authentication type for SurrealDB
    #[clap(
        short = 'a',
        long,
        // default_value = "none",
        env = "SURREAL_AUTH_TYPE",
        // group = "surrealdb"
    )]
    pub auth_type: Option<SurrealAuthType>,

    /// Token for authentication
    /// Required if `auth_type` is `Token`
    #[clap(
        short = 'k',
        long,
        required_if_eq("auth_type", "Token"),
        env = "SURREAL_TOKEN",
        group = "surrealdb"
    )]
    pub token: Option<String>,
    /// Username for authentication
    /// Required if `auth_type` is `Root` or `Namespace`
    #[clap(
        short = 'u',
        long,
        required_if_eq_any([("auth_type", "Root"), ("auth_type", "Namespace")]),
        env = "SURREAL_USER",
        // group = "surrealdb"
    )]
    pub username: Option<String>,
    /// Password for authentication
    /// Required if `auth_type` is `UsernamePassword`
    /// This field is marked as `sensitive` so the value will be redacted in logs
    #[clap(
        short = 'p',
        long,
        required_if_eq_any([("auth_type", "Root"), ("auth_type", "Namespace")]),
        env = "SURREAL_PASS",
        // group = "surrealdb"
    )]
    pub password: Option<String>,

    /// Namespace to use in SurrealDB
    #[clap(
        short = 'n',
        long,
        default_value = "bsky.network",
        env = "SURREAL_NAMESPACE"
    )]
    pub namespace: String,

    /// Database to use in SurrealDB
    #[clap(short = 'd', long, default_value = "bsky", env = "SURREAL_DATABASE")]
    pub database: String,

    /// Fetch user data from ATProto
    #[clap(short = 'f', long, env = "FETCH_USER_DATA", default_value = "true")]
    pub fetch_user_data: bool,
}

const FETCH_DATA_ENVAR: &str = "_FETCH_USER_DATA";

/// HACK: Set internal envar for fetching user data when using SurrealDB exporter
///
/// Disabling this will prevent the exporter from fetching data of every user in the stream,
/// which is computationally expensive and not necessary for most use cases.
///
/// This is enabled by default, see `[SurrealDbConn.fetch_user_data]` for more information.
pub(crate) fn will_fetch_user_data() -> bool {
    std::env::var(FETCH_DATA_ENVAR)
        .map(|v| v == "true")
        .unwrap_or(false)
}

impl SurrealDbConn {
    pub async fn get_surreal_conn(&self) -> Result<Surreal<surrealdb::engine::any::Any>> {
        let endpoint = self.surreal_endpoint.clone();
        let conn: Surreal<surrealdb::engine::any::Any> = Surreal::init();
        conn.connect(endpoint).await?;

        match self.auth_type {
            Some(SurrealAuthType::Token) => {
                conn.authenticate(self.token.as_deref().unwrap()).await?;
            }
            Some(SurrealAuthType::Root) => {
                conn.signin(Root {
                    username: self.username.as_deref().unwrap(),
                    password: self.password.as_deref().unwrap(),
                })
                .await?;
            }
            Some(SurrealAuthType::Namespace) => {
                conn.signin(surrealdb::opt::auth::Namespace {
                    username: self.username.as_deref().unwrap(),
                    password: self.password.as_deref().unwrap(),
                    namespace: &self.namespace.clone(),
                })
                .await?;
            }
            _ => {}
        }

        if self.fetch_user_data {
            std::env::set_var(FETCH_DATA_ENVAR, "true");
        }

        conn.use_ns(&self.namespace).use_db(&self.database).await?;

        // run schema
        let schema = include_str!("schema.surql");
        tracing::info!("Loading schema");
        conn.query(schema).await?;

        Ok(conn)
    }
}

#[derive(Parser, Debug)]
#[clap(name = "skystreamer", about = "A tool for streaming data to SurrealDB")]
pub struct Config {
    /// SurrealDB endpoint
    #[clap(flatten)]
    pub surreal_conn: SurrealDbConn,
    #[clap(short = 'E', long, default_value = "surrealdb", env = "EXPORTER")]
    pub exporter: ExporterType,
    #[clap(flatten)]
    pub file_exporter: FileExporterOptions,

    #[clap(short = 'R', long, default_value = "bsky.network", env = "ATPROTO_RELAY")]
    pub atproto_relay: String,
}

impl Config {
    pub async fn consumer(&self) -> Result<Consumer> {
        let exporter = match self.exporter {
            ExporterType::Jsonl => {
                let file_path = self.file_exporter.file_path.as_ref().unwrap();
                let file = tokio::fs::File::create(file_path).await?;
                Box::new(crate::exporter::JsonlExporter::new(file))
                    as Box<dyn crate::exporter::Exporter>
            }
            ExporterType::Csv => {
                let file_path = self.file_exporter.file_path.as_ref().unwrap();
                let file = tokio::fs::File::create(file_path).await?;
                Box::new(crate::exporter::CsvExporter::new(file))
                    as Box<dyn crate::exporter::Exporter>
            }
            ExporterType::Surrealdb => {
                let conn = self.surreal_conn.get_surreal_conn().await?;
                Box::new(crate::exporter::SurrealDbExporter::new(conn))
                    as Box<dyn crate::exporter::Exporter>
            }
            ExporterType::DryRun => {
                Box::new(crate::exporter::DryRunExporter) as Box<dyn crate::exporter::Exporter>
            }
        };

        Ok(Consumer::new(exporter, &self.atproto_relay))
    }
}
