use crate::Result;
use atrium_api::{
    app::bsky::{
        embed::{
            images::ImageData, record_with_media::MainMediaRefs, video::MainData as VideoData,
        },
        feed::post::{Record as PostRecord, RecordEmbedRefs},
    },
    com::atproto::sync::subscribe_repos::Commit,
    types::{string::Did, BlobRef, CidLink, TypedBlobRef, UnTypedBlobRef},
};
use cid::multihash::Multihash;
use cid::Cid;
use ipld_core::ipld::Ipld;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::io::Cursor;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Media {
    // Image(ImageData),
    Image(Image),
    // note: weird naming
    // Video(VideoData),
    Video(Video),
}

// We have to define our own wrapper types because
// BlobRef union type are kinda weird
// todo: Figure out how to fix this

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Video {
    /// Alt text
    pub alt: Option<String>,
    pub blob: Blob,
    pub aspect_ratio: Option<(u32, u32)>,
}

impl From<VideoData> for Video {
    fn from(value: VideoData) -> Self {
        Self {
            alt: value.alt,
            blob: value.video.into(),
            aspect_ratio: value
                .aspect_ratio
                .map(|ar| (ar.width.get() as u32, ar.height.get() as u32)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Image {
    pub alt: String,
    pub blob: Blob,
    pub aspect_ratio: Option<(u32, u32)>,
}

impl From<ImageData> for Image {
    fn from(value: ImageData) -> Self {
        Self {
            alt: value.alt,
            blob: value.image.into(),
            aspect_ratio: value
                .aspect_ratio
                .map(|ar| (ar.width.get() as u32, ar.height.get() as u32)),
        }
    }
}
// pub struct ExternalData {
//     pub description: String,
//     #[serde(skip_serializing_if = "core::option::Option::is_none")]
//     pub thumb: core::option::Option<crate::types::BlobRef>,
//     pub title: String,
//     pub uri: String,
// }
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalLink {
    pub description: String,
    pub thumb: Option<Blob>,
    pub title: String,
    pub uri: String,
}

impl From<atrium_api::app::bsky::embed::external::ExternalData> for ExternalLink {
    fn from(value: atrium_api::app::bsky::embed::external::ExternalData) -> Self {
        Self {
            description: value.description,
            thumb: value.thumb.map(|b| b.into()),
            title: value.title,
            uri: value.uri,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Blob {
    pub cid: String,
    pub mime_type: String,
    pub size: Option<usize>,
}

impl From<BlobRef> for Blob {
    fn from(value: BlobRef) -> Self {
        match value {
            BlobRef::Typed(TypedBlobRef::Blob(blob)) => Self {
                cid: blob.r#ref.0.to_string(),
                mime_type: blob.mime_type,
                size: Some(blob.size),
            },
            BlobRef::Untyped(UnTypedBlobRef { cid, mime_type }) => Self {
                cid,
                mime_type,
                size: None,
            },
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplyRef {
    pub parent: Cid,
    pub root: Cid,
}

impl From<atrium_api::app::bsky::feed::post::ReplyRef> for ReplyRef {
    fn from(value: atrium_api::app::bsky::feed::post::ReplyRef) -> Self {
        Self {
            parent: value.parent.data.cid.as_ref().to_owned(),
            root: value.root.data.cid.as_ref().to_owned(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Embed {
    Images(Vec<Image>),
    External(ExternalLink),
    Record(Cid),
    RecordWithMedia(Cid, Box<Vec<Media>>),
    Unknown,
}

impl From<RecordEmbedRefs> for Embed {
    fn from(value: RecordEmbedRefs) -> Self {
        match value {
            RecordEmbedRefs::AppBskyEmbedImagesMain(m) => Embed::Images(
                m.images
                    .clone()
                    .into_iter()
                    .map(|i| i.data.into())
                    .collect(),
            ),
            RecordEmbedRefs::AppBskyEmbedExternalMain(m) => {
                Embed::External(m.data.external.data.into())
            }
            RecordEmbedRefs::AppBskyEmbedRecordMain(m) => {
                Embed::Record(m.record.data.cid.as_ref().to_owned())
            }
            RecordEmbedRefs::AppBskyEmbedRecordWithMediaMain(m) => {
                let media = match &m.media {
                    atrium_api::types::Union::Refs(MainMediaRefs::AppBskyEmbedImagesMain(m)) => m
                        .images
                        .clone()
                        .into_iter()
                        .map(|i| Media::Image(i.data.into()))
                        .collect::<Vec<Media>>(),
                    atrium_api::types::Union::Refs(MainMediaRefs::AppBskyEmbedVideoMain(m)) => {
                        vec![Media::Video(m.data.clone().into())]
                    }
                    _ => return Embed::Unknown,
                };
                // let media = match m.media {
                //     atrium_api::types::Union::Refs(MainMediaRefs::AppBskyEmbedImagesMain(m)) => {
                //         // Media::Image(
                //             m.images
                //                 .into_iter()
                //                 .map(|i| i.data.into())
                //                 .collect::<Vec<Image>>(),
                //         // )
                //     }
                //     MainMediaRefs::AppBskyEmbedVideoMain(m) => Media::Video(m.data.into()),
                //     _ => return Embed::Unknown,
                // };
                Embed::RecordWithMedia(
                    m.record.data.record.cid.as_ref().to_owned(),
                    Box::new(media),
                )
            }
            _ => Embed::Unknown,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Post {
    pub author: Did,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub text: String,
    pub id: Cid,
    // pub cid: String,
    pub language: Vec<String>,
    pub reply: Option<ReplyRef>,
    pub tags: Vec<String>,
    pub labels: Vec<String>,
    pub embed: Option<Embed>,
}

impl From<PostData> for Post {
    fn from(value: PostData) -> Self {
        let record = value.record.data;
        Self {
            author: value.author,
            // because for some reason we can't access the inner chrono::DateTime
            // we will have to reparse it from string
            created_at: record.created_at.as_str().parse().unwrap(),
            text: record.text,
            id: value.cid,
            language: record
                .langs
                .as_ref()
                .map(|langs| langs.iter().map(|lang| lang.as_ref().to_string()).collect())
                .unwrap_or_default(),
            reply: record.reply.as_ref().map(|reply| (*reply).clone().into()),
            tags: record.tags.unwrap_or_default().iter().map(|tag| tag.to_string()).collect(),
            labels: record.labels.as_ref().map_or_else(Vec::new, |labels| {
                match labels {
                    atrium_api::types::Union::Refs(
                        atrium_api::app::bsky::feed::post::RecordLabelsRefs::ComAtprotoLabelDefsSelfLabels(
                            values,
                        ),
                    ) => values.data.values.iter().map(|label| label.val.as_str().to_string()).collect(),
                    _ => Vec::new(),
                }
            }),
            embed: record.embed.as_ref().map(|embed| match embed {
                atrium_api::types::Union::Refs(refs) => refs.clone().into(),
                _ => Embed::Unknown,
            }),
        }
    }
}

// impl From<atrium_api::com::atproto::repo::strong_ref::MainData> for Post {
//     fn from(value: atrium_api::com::atproto::repo::strong_ref::MainData) -> Self {
//         Self {
//             author: value.author,
//             created_at: value.created_at,
//             text: value.text,
//             id: value.id,
//             cid: value.cid,
//             language: value.language,
//             reply: value.reply.map(|r| ReplyRef {
//                 parent: r.parent,
//                 root: r.root,
//             }),
//             tags: value.tags,
//             labels: value.labels,
//             embed: value.embed.map(|e| e.into()),
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostData {
    pub author: Did,
    pub cid: Cid,
    pub record: PostRecord,
}

impl PostData {
    pub fn new(author: Did, cid: CidLink, record: PostRecord) -> Self {
        Self {
            author,
            cid: cid.0,
            record,
        }
    }

    /// Get media associated with the post
    pub fn get_media(&self) -> Option<Vec<Media>> {
        let embed = self.record.embed.as_ref()?;

        match embed {
            atrium_api::types::Union::Refs(RecordEmbedRefs::AppBskyEmbedImagesMain(m)) => Some(
                m.images
                    .iter()
                    .map(|i| Media::Image(i.data.clone().into()))
                    .collect(),
            ),
            atrium_api::types::Union::Refs(RecordEmbedRefs::AppBskyEmbedVideoMain(m)) => {
                Some(vec![Media::Video(m.data.clone().into())])
            }
            atrium_api::types::Union::Refs(RecordEmbedRefs::AppBskyEmbedRecordWithMediaMain(m)) => {
                match &m.media {
                    atrium_api::types::Union::Refs(MainMediaRefs::AppBskyEmbedImagesMain(m)) => {
                        Some(
                            m.images
                                .iter()
                                .map(|i| Media::Image(i.data.clone().into()))
                                .collect(),
                        )
                    }
                    atrium_api::types::Union::Refs(MainMediaRefs::AppBskyEmbedVideoMain(m)) => {
                        Some(vec![Media::Video(m.data.clone().into())])
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // pub fn
}

pub async fn download_media<C>(
    client: &atrium_api::client::Service<C>,
    did: &Did,
    media: &Media,
) -> Result<Vec<u8>>
where
    C: atrium_api::xrpc::XrpcClient + Send + Sync,
{
    let blob_ref = match media {
        Media::Image(data) => &data.blob,
        Media::Video(data) => &data.blob,
    };

    let bytes = client
        .com
        .atproto
        .sync
        .get_blob(
            atrium_api::com::atproto::sync::get_blob::ParametersData {
                cid: atrium_api::types::string::Cid::new(blob_ref.cid.clone().parse().unwrap()),
                did: did.clone(),
            }
            .into(),
        )
        .await
        .map_err(|e| crate::Error::AtriumError(e.to_string()))?;
    Ok(bytes)
}
// original definition:
//```
// export enum FrameType {
//   Message = 1,
//   Error = -1,
// }
// export const messageFrameHeader = z.object({
//   op: z.literal(FrameType.Message), // Frame op
//   t: z.string().optional(), // Message body type discriminator
// })
// export type MessageFrameHeader = z.infer<typeof messageFrameHeader>
// export const errorFrameHeader = z.object({
//   op: z.literal(FrameType.Error),
// })
// export type ErrorFrameHeader = z.infer<typeof errorFrameHeader>
// ```
#[derive(Debug, Clone, PartialEq, Eq)]
enum FrameHeader {
    Message(Option<String>),
    Error,
}

impl TryFrom<Ipld> for FrameHeader {
    type Error = crate::Error;

    fn try_from(value: Ipld) -> Result<Self> {
        if let Ipld::Map(map) = &value {
            if let Some(Ipld::Integer(i)) = map.get("op") {
                match i {
                    1 => {
                        let t = if let Some(Ipld::String(s)) = map.get("t") {
                            Some(s.clone())
                        } else {
                            None
                        };
                        return Ok(FrameHeader::Message(t));
                    }
                    -1 => return Ok(FrameHeader::Error),
                    _ => {}
                }
            }
        }
        Err(crate::Error::InvalidFrameType(value).into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Message(Option<String>, MessageFrame),
    Error(ErrorFrame),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageFrame {
    pub body: Vec<u8>,
}

#[trait_variant::make(HttpService: Send)]
pub trait Subscription {
    async fn next(
        &mut self,
    ) -> Option<std::result::Result<Frame, <Frame as TryFrom<&[u8]>>::Error>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorFrame {
    // TODO
    // body: Value,
}

impl TryFrom<&[u8]> for Frame {
    type Error = crate::Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(value);
        let (left, right) = match serde_ipld_dagcbor::from_reader::<Ipld, _>(&mut cursor) {
            Err(serde_ipld_dagcbor::DecodeError::TrailingData) => {
                value.split_at(cursor.position() as usize)
            }
            _ => {
                // TODO
                return Err(crate::Error::InvalidFrameData(value.to_vec()).into());
            }
        };
        let header = FrameHeader::try_from(serde_ipld_dagcbor::from_slice::<Ipld>(left)?)?;
        if let FrameHeader::Message(t) = &header {
            Ok(Frame::Message(
                t.clone(),
                MessageFrame {
                    body: right.to_vec(),
                },
            ))
        } else {
            Ok(Frame::Error(ErrorFrame {}))
        }
    }
}

pub struct CidOld(cid_old::Cid);

impl From<cid_old::Cid> for CidOld {
    fn from(value: cid_old::Cid) -> Self {
        Self(value)
    }
}
impl TryFrom<CidOld> for Cid {
    type Error = cid::Error;
    fn try_from(value: CidOld) -> std::result::Result<Self, Self::Error> {
        let version = match value.0.version() {
            cid_old::Version::V0 => cid::Version::V0,
            cid_old::Version::V1 => cid::Version::V1,
        };

        let codec = value.0.codec();
        let hash = value.0.hash();
        let hash = Multihash::from_bytes(&hash.to_bytes())?;

        Self::new(version, codec, hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialized_data(s: &str) -> Vec<u8> {
        assert!(s.len() % 2 == 0);
        let b2u = |b: u8| match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            _ => unreachable!(),
        };
        s.as_bytes()
            .chunks(2)
            .map(|b| (b2u(b[0]) << 4) + b2u(b[1]))
            .collect()
    }

    #[test]
    fn deserialize_message_frame_header() {
        // {"op": 1, "t": "#commit"}
        let data = serialized_data("a2626f700161746723636f6d6d6974");
        let ipld = serde_ipld_dagcbor::from_slice::<Ipld>(&data).expect("failed to deserialize");
        let result = FrameHeader::try_from(ipld);
        assert_eq!(
            result.expect("failed to deserialize"),
            FrameHeader::Message(Some(String::from("#commit")))
        );
    }

    #[test]
    fn deserialize_error_frame_header() {
        // {"op": -1}
        let data = serialized_data("a1626f7020");
        let ipld = serde_ipld_dagcbor::from_slice::<Ipld>(&data).expect("failed to deserialize");
        let result = FrameHeader::try_from(ipld);
        assert_eq!(result.expect("failed to deserialize"), FrameHeader::Error);
    }

    #[test]
    fn deserialize_invalid_frame_header() {
        {
            // {"op": 2, "t": "#commit"}
            let data = serialized_data("a2626f700261746723636f6d6d6974");
            let ipld =
                serde_ipld_dagcbor::from_slice::<Ipld>(&data).expect("failed to deserialize");
            let result = FrameHeader::try_from(ipld);
            assert_eq!(
                result.expect_err("must be failed").to_string(),
                "invalid frame type"
            );
        }
        {
            // {"op": -2}
            let data = serialized_data("a1626f7021");
            let ipld =
                serde_ipld_dagcbor::from_slice::<Ipld>(&data).expect("failed to deserialize");
            let result = FrameHeader::try_from(ipld);
            assert_eq!(
                result.expect_err("must be failed").to_string(),
                "invalid frame type"
            );
        }
    }
}
