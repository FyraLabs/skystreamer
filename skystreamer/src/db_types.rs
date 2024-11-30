use crate::{exporter::{POSTS_TABLE, USERS_TABLE}, types::{Blob, ExternalLink, Media, PostData}};
use atrium_api::{app::bsky::actor::defs::ProfileViewDetailedData, types::string::Did};
use serde::{Deserialize, Serialize};
use surrealdb::{sql::Thing, RecordId};
use url::Url;

// example post:

// {
// 	author: 'did:plc:fn5fmoghtuypw2gka67oqs7p',
// 	created_at: '2024-11-29T03:01:11.389Z',
// 	embed: {
// 		external: [
// 			{
// 				description: '',
// 				py_type: 'app.bsky.embed.external#external',
// 				thumb: NULL,
// 				title: 'Redirecting...',
// 				uri: 'https://www.facebook.com/share/v/14z4vJf6kQ/?mibextid=WC7FNe'
// 			}
// 		],
// 		images: [],
// 		record: [],
// 		videos: []
// 	},
// 	id: post:bafyreia6v4obvevii54u3itb5zwq6pg56fc3qq5try3wct4s427zs3fvle,
// 	labels: [],
// 	language: 'en',
// 	post_id: '3lc2looeiuc2n',
// 	reply: NULL,
// 	tags: NULL,
// 	text: 'www.facebook.com/share/v/14z4...

// www.facebook.com/share/v/14z4...'
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplyRef {
    /// Parent CID of the post
    pub reply_parent: RecordId,
    /// Root of reply
    pub reply_root: RecordId,
}
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Embed {
    /// External links associated with post
    pub external_links: Option<Vec<ExternalLink>>,
    /// Media associated with post
    pub media: Option<Vec<Media>>,
    /// Link to another Post quoted by the current post
    pub quote: Option<RecordId>,
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
            avatar: profile.avatar.as_ref().map(|url| parse_bsky_img_url(Url::parse(url).unwrap())),
            labels: profile.labels.as_ref().map(|labels| labels.iter().map(|label| label.val.as_str().to_string()).collect()).unwrap_or_default(),
            created_at: profile.created_at.clone().map(|date| date.as_str().parse().unwrap()),
            indexed_at: profile.indexed_at.clone().map(|date| date.as_str().parse().unwrap()),
            banner: profile.banner.as_ref().map(|url| parse_bsky_img_url(Url::parse(url).unwrap())),
            description: profile.description.clone(),
            followers: profile.followers_count.map(|count| count as usize),
            following: profile.follows_count.map(|count| count as usize),
            posts: profile.posts_count.map(|count| count as usize),
        }
    }
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Post {
    /// Author of post
    ///
    /// DID of the author
    ///
    /// The `did:plc:` prefix is stripped from the DID for brevity.
    /// To get the full DID using ATProto, you can prepend it back.
    pub author: surrealdb::value::RecordId,

    #[serde(skip)]
    pub author_did: Option<Did>,

    /// Date and time of post creation
    pub created_at: surrealdb::sql::Datetime,
    /// Text content of post
    pub text: String,
    /// CID of post
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,

    #[serde(skip)]
    pub cid: String,
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

impl Post {
    #[tracing::instrument]
    pub fn new(data: PostData) -> Self {
        Post {
            // author: data.author.to_string().trim_start_matches("did:plc:").to_string(),
            author: RecordId::from_table_key(USERS_TABLE, data.author.to_string().trim_start_matches("did:plc:").to_string()),
            author_did: Some(data.author.clone()), 
            id: data.cid.to_string().parse().ok(),
            cid: data.cid.to_string(),
            created_at: data.record.created_at.as_str().parse().unwrap(),
            language: data
                .record
                .langs
                .as_ref()
                .map(|langs| langs.iter().map(|lang| lang.as_ref().to_string()).collect())
                .unwrap_or_default(),
            text: data.record.text.clone(),
            reply: data.record.reply.as_ref().map(|reply| ReplyRef {
                reply_parent: RecordId::from_table_key(POSTS_TABLE, reply.parent.cid.as_ref().to_string()),
                reply_root: RecordId::from_table_key(POSTS_TABLE, reply.root.cid.as_ref().to_string()),
            }),
            labels: data
                .record
                .labels
                .as_ref()
                .map_or_else(Vec::new, |labels| match labels {
                    atrium_api::types::Union::Refs(
                        atrium_api::app::bsky::feed::post::RecordLabelsRefs::ComAtprotoLabelDefsSelfLabels(
                            values,
                        ),
                    ) => values.data.values.iter().map(|label| label.val.as_str().to_string()).collect(),
                    _ => Vec::new(),
                }),
            tags: data.record.tags.as_ref().map_or_else(Vec::new, |tags| tags.iter().map(|tag| tag.to_string()).collect()),
            embed: data.record.embed.as_ref().map(|embed| {
                // let mut media: Vec<Media> = Vec::new();
                let embedded_media = data.get_media();
                let external_links = match &embed {
                    atrium_api::types::Union::Refs(
                        atrium_api::app::bsky::feed::post::RecordEmbedRefs::AppBskyEmbedExternalMain(
                            external,
                        ),
                    ) => Some(vec![external.data.clone().external.data]),
                    _ => None,
                };

                let quote = match &embed {
                    atrium_api::types::Union::Refs(
                        atrium_api::app::bsky::feed::post::RecordEmbedRefs::AppBskyEmbedRecordWithMediaMain(embed_data)
                    ) => Some(&embed_data.record.data.record.cid),
                    atrium_api::types::Union::Refs(
                        atrium_api::app::bsky::feed::post::RecordEmbedRefs::AppBskyEmbedRecordMain(embed_data)
                    ) => Some(&embed_data.data.record.cid),
                    _ => None,
                };

                Embed {
                    external_links: external_links.map(|external_links| {
                        external_links
                            .iter()
                            .map(|external_link| external_link.to_owned().into())
                            .collect()
                    }),
                    media: Some(embedded_media.unwrap_or_default()),
                    quote: quote.map(|quote| RecordId::from_table_key(POSTS_TABLE, quote.as_ref().to_string())),
                }
            }),
        }
        // todo!()
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