use atrium_api::{app::bsky, com::atproto::sync::subscribe_repos::RepoOp, types::{CidLink, Collection}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    // app.bsky.feed.post
    Post(Option<CidLink>, RepoOp),
    // app.bsky.graph.follow
    Follow(Option<CidLink>, RepoOp),
    // app.bsky.graph.block
    Block(Option<CidLink>, RepoOp),
    // app.bsky.feed.repost
    Repost(Option<CidLink>, RepoOp),
    // app.bsky.feed.like
    Like(Option<CidLink>, RepoOp),
    // app.bsky.graph.listitem
    ListItem(Option<CidLink>, RepoOp),
    // app.bsky.actor.profile
    Profile(Option<CidLink>, RepoOp),
    // other stuff
    Other(String, Option<CidLink>, RepoOp),
}

impl Operation {
    pub fn from_op(op: RepoOp) -> Self {
        let path = op.path.split('/').next().unwrap_or_default();
        let cid = op.cid.clone();
        match path {
            bsky::feed::Post::NSID => Operation::Post(cid, op),
            bsky::graph::Follow::NSID => Operation::Follow(cid, op),
            bsky::graph::Block::NSID => Operation::Block(cid, op),
            bsky::feed::Repost::NSID => Operation::Repost(cid, op),
            bsky::feed::Like::NSID => Operation::Like(cid, op),
            bsky::graph::Listitem::NSID => Operation::ListItem(cid, op),
            bsky::actor::Profile::NSID => Operation::Profile(cid, op),
            _ => Operation::Other(path.to_string(), cid, op),
        }
    }

    pub fn get_cid(&self) -> Option<CidLink> {
        match self {
            Operation::Post(cid, _)
            | Operation::Follow(cid, _)
            | Operation::Block(cid, _)
            | Operation::Repost(cid, _)
            | Operation::Like(cid, _)
            | Operation::ListItem(cid, _)
            | Operation::Profile(cid, _)
            | Operation::Other(_, cid, _) => cid.clone(),
        }
    }

    pub fn get_op(&self) -> RepoOp {
        match self {
            Operation::Post(_, op)
            | Operation::Follow(_, op)
            | Operation::Block(_, op)
            | Operation::Repost(_, op)
            | Operation::Like(_, op)
            | Operation::ListItem(_, op)
            | Operation::Profile(_, op)
            | Operation::Other(_, _, op) => op.clone(),
        }
    }
}
