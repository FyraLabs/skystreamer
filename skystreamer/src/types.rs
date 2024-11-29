use atrium_api::app::bsky::embed::images::ImageData;
use atrium_api::app::bsky::embed::record_with_media::MainMediaRefs;
use atrium_api::app::bsky::embed::video::MainData as VideoData;
use atrium_api::app::bsky::feed::post::{Record as PostRecord, RecordEmbedRefs};
use atrium_api::com::atproto::sync::subscribe_repos::Commit;
use atrium_api::types::string::Did;
use atrium_api::types::{BlobRef, CidLink, TypedBlobRef, UnTypedBlobRef};
use cid::multihash::Multihash;
use cid::Cid;
use color_eyre::Result;
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

#[derive(Debug, Serialize, Deserialize)]
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
        .await?;
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
    type Error = color_eyre::eyre::Error;

    fn try_from(value: Ipld) -> Result<Self, <FrameHeader as TryFrom<Ipld>>::Error> {
        if let Ipld::Map(map) = value {
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
        Err(color_eyre::eyre::eyre!("invalid frame type"))
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
    async fn next(&mut self) -> Option<Result<Frame, <Frame as TryFrom<&[u8]>>::Error>>;
}

pub trait CommitHandler {
    fn handle_commit(&mut self, commit: &Commit) -> impl Future<Output = Result<()>>;
    fn update_cursor(&self, seq: u64) -> impl Future<Output = Result<()>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorFrame {
    // TODO
    // body: Value,
}

impl TryFrom<&[u8]> for Frame {
    type Error = color_eyre::eyre::Error;

    fn try_from(value: &[u8]) -> Result<Self, <Frame as TryFrom<&[u8]>>::Error> {
        let mut cursor = Cursor::new(value);
        let (left, right) = match serde_ipld_dagcbor::from_reader::<Ipld, _>(&mut cursor) {
            Err(serde_ipld_dagcbor::DecodeError::TrailingData) => {
                value.split_at(cursor.position() as usize)
            }
            _ => {
                // TODO
                return Err(color_eyre::eyre::eyre!("invalid frame type"));
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
    fn try_from(value: CidOld) -> Result<Self, Self::Error> {
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
