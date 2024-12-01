// use crate::{types::{Blob, ExternalLink, Media, PostData}};
use atrium_api::{app::bsky::actor::defs::ProfileViewDetailedData, types::string::Did};
use serde::{Deserialize, Serialize};
use skystreamer::types::{Blob, ExternalLink, Media};
use surrealdb::{sql::Thing, RecordId};
use url::Url;

use crate::exporter::{POSTS_TABLE, USERS_TABLE};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplyRef {
    /// Parent CID of the post
    pub reply_parent: RecordId,
    /// Root of reply
    pub reply_root: RecordId,
}
// #[derive(Debug, Serialize, Deserialize, Default, Clone)]
// pub struct Embed {
//     /// External links associated with post
//     pub external_links: Option<Vec<ExternalLink>>,
//     /// Media associated with post
//     pub media: Option<Vec<Media>>,
//     /// Link to another Post quoted by the current post
//     pub quote: Option<RecordId>,
// }
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Embed {
    Media(Vec<Media>),
    External(ExternalLink),
    Record(RecordId),
    RecordWithMedia { record: RecordId, media: Vec<Media> },
    Unknown,
}

impl From<&skystreamer::types::Embed> for Embed {
    fn from(embed: &skystreamer::types::Embed) -> Self {
        match embed {
            skystreamer::types::Embed::Media(media) => Embed::Media(media.clone()),
            skystreamer::types::Embed::External(link) => Embed::External(link.clone()),
            skystreamer::types::Embed::Record(record) => {
                Embed::Record(RecordId::from_table_key(POSTS_TABLE, record.to_string()))
            }
            skystreamer::types::Embed::RecordWithMedia(record, media) => Embed::RecordWithMedia {
                record: RecordId::from_table_key(POSTS_TABLE, record.to_string()),
                media: *media.clone(),
            },
            _ => Embed::Unknown,
        }
    }
}

fn parse_bsky_img_url(url: Url) -> Blob {
    // https://cdn.bsky.app/img/avatar/plain/did:plc:mmb3snayvehyjxlngslg4bci/bafkreigaitwqbx65ol4b4phtorimubooiub4z57arjk5vvklczoztn5q6i@jpeg"

    // split until the last 2 parts

    let mut parts = url.path().split('/').collect::<Vec<&str>>();

    let cid_blob = parts.pop().unwrap();
    let _did = parts.pop().unwrap();

    // we should be able to just prepend the `did:plc:` prefix to the DID

    let (cid, ext) = cid_blob.split_once('@').unwrap();

    Blob {
        size: None,
        cid: cid.to_string(),
        mime_type: format!("image/{}", ext),
    }
}

// A Bluesky user profile, A minimized version of ProfileViewBasic
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct User {
    /// DID of the user
    #[serde(skip)]
    pub did: Option<Did>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,

    /// Display name of user
    pub display_name: Option<String>,
    /// Handle/Domain name of user
    pub handle: String,
    /// Avatar of user
    pub avatar: Option<Blob>,
    /// Labels associated with user
    pub labels: Vec<String>,
    /// Date and time of user creation
    pub created_at: Option<surrealdb::sql::Datetime>,
    /// Date and time of user last indexed
    pub indexed_at: Option<surrealdb::sql::Datetime>,
    /// Banner image of user
    pub banner: Option<Blob>,
    /// Bio/description of user
    pub description: Option<String>,
    /// Number of followers of this user has
    pub followers: Option<usize>,
    /// Number of users this user follows
    pub following: Option<usize>,
    /// Number of total posts created by this user
    pub posts: Option<usize>,
}

impl From<ProfileViewDetailedData> for User {
    fn from(profile: ProfileViewDetailedData) -> Self {
        User {
            did: Some(profile.did.clone()),
            id: None,
            display_name: profile.display_name.clone(),
            handle: profile.handle.to_string(),
            avatar: profile
                .avatar
                .as_ref()
                .map(|url| parse_bsky_img_url(Url::parse(url).unwrap())),
            labels: profile
                .labels
                .as_ref()
                .map(|labels| {
                    labels
                        .iter()
                        .map(|label| label.val.as_str().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            created_at: profile
                .created_at
                .clone()
                .map(|date| date.as_str().parse().unwrap()),
            indexed_at: profile
                .indexed_at
                .clone()
                .map(|date| date.as_str().parse().unwrap()),
            banner: profile
                .banner
                .as_ref()
                .map(|url| parse_bsky_img_url(Url::parse(url).unwrap())),
            description: profile.description.clone(),
            followers: profile.followers_count.map(|count| count as usize),
            following: profile.follows_count.map(|count| count as usize),
            posts: profile.posts_count.map(|count| count as usize),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SurrealPostRep {
    /// Author of post
    ///
    /// DID of the author
    ///
    /// The `did:plc:` prefix is stripped from the DID for brevity.
    /// To get the full DID using ATProto, you can prepend it back.
    pub author: surrealdb::value::RecordId,

    /// Date and time of post creation
    pub created_at: surrealdb::sql::Datetime,
    /// Text content of post
    pub text: String,
    /// CID of post
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    /// Language(s) of post
    ///
    pub language: Vec<String>,
    /// Reference to reply
    pub reply: Option<ReplyRef>,
    /// Tags associated with post
    pub tags: Vec<String>,
    /// Labels associated with post
    pub labels: Vec<String>,
    pub embed: Option<Embed>,
}

impl From<skystreamer::types::Post> for SurrealPostRep {
    fn from(post: skystreamer::types::Post) -> Self {
        // get current timezone
        let now = chrono::Local::now();
        let now_tz = now.timezone();
        // post.created_at.offset();

        // let time_now_utc = chrono::Utc::now().naive_local();
        // let created_at = post.created_at.to_utc();
        let created_at: chrono::DateTime<chrono::Utc> = post.created_at.to_utc();
        let created_at_local = created_at.with_timezone(&now_tz);
        // tracing::info!("Time: {:?}", post.created_at);
        // check if time is some reason in the future
        if created_at_local > now {
            tracing::warn!("Post created_at is in the future!?: {:?}", created_at);
        }
        SurrealPostRep {
            author: RecordId::from_table_key(
                USERS_TABLE,
                post.author
                    .to_string()
                    .strip_prefix("did:plc:")
                    .unwrap()
                    .to_string(),
            ),
            created_at: post.created_at.to_utc().into(),
            text: post.text,
            id: None, // This will be filled in later by the exporter
            embed: post.embed.as_ref().map(|embed| embed.into()),
            language: post.language,
            reply: post.reply.as_ref().map(|reply| ReplyRef {
                reply_parent: RecordId::from_table_key(POSTS_TABLE, reply.parent.to_string()),
                reply_root: RecordId::from_table_key(POSTS_TABLE, reply.root.to_string()),
            }),
            tags: post.tags,
            labels: post.labels,
        }
    }
}
#[cfg(test)]
mod tests {
    pub use super::*;
    const CAPPY_PFP_URL: &str = "https://cdn.bsky.app/img/avatar/plain/did:plc:x4pssacf24wuotdl65zntnsr/bafkreihsq6kzrgb2jzyg3jowj4bfw5hwoh2dx7zagcplh5ooe2b5cdgche@jpeg";
    // const CAPPY_DID: &str = "did:plc:x4pssacf24wuotdl65zntnsr";

    const PUNIRU_CID: &str = "bafkreihsq6kzrgb2jzyg3jowj4bfw5hwoh2dx7zagcplh5ooe2b5cdgche";

    #[test]
    fn test_parse_bsky_img_url() {
        let url = Url::parse(CAPPY_PFP_URL).unwrap();
        let blob = parse_bsky_img_url(url);

        println!("{:#?}", blob);
        assert_eq!(blob.cid, PUNIRU_CID);
        assert_eq!(blob.mime_type, "image/jpeg");
    }
}
