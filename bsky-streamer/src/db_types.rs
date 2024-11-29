use crate::types::PostData;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplyRef {
    /// Parent CID of the post
    pub parent: String,
    /// Root of reply
    pub root: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Post {
    /// Author of post
    ///
    /// DID of the author
    pub author: String,
    /// Date and time of post creation
    pub created_at: DateTime<chrono::Utc>,
    /// Text content of post
    pub text: String,
    /// CID of post
    pub id: String,
    /// Language of post
    ///
    /// While this is defined as an array, it usually contains only one element
    /// so we can safely assume it's a single string
    pub language: String,
    /// Reference to reply
    pub reply: Option<ReplyRef>,
    /// Tags associated with post
    pub tags: Vec<String>,
    /// Labels associated with post
    pub labels: Vec<String>,
}

impl Post {
    #[tracing::instrument]
    pub fn new(data: PostData) -> Self {
        Post {
            author: data.author.to_string(),
            id: data.cid.to_string(),
            created_at: data.record.created_at.as_str().parse().unwrap(),
            language: data
                .record
                .langs
                .as_ref()
                .and_then(|langs| langs.first().map(|lang| lang.as_ref().to_string()))
                .unwrap_or_default(),
            text: data.record.text.clone(),
            reply: data.record.reply.as_ref().map(|reply| ReplyRef {
                parent: reply.parent.cid.as_ref().to_string(),
                root: reply.root.cid.as_ref().to_string(),
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
                        
                    ) => values.data.values.clone().into_iter().map(|label| label.val.clone()).collect(),
                    _ => Vec::new(),
                }),
            tags: data.record.tags.clone().unwrap_or_default()
        }
        // todo!()
    }
}
