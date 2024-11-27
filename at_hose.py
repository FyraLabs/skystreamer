import asyncio
import os
import signal
import time
from collections import defaultdict
from types import FrameType
from typing import Any
import logging

# import json
from surrealdb import AsyncSurrealDB


from atproto import (
    CAR,
    AsyncFirehoseSubscribeReposClient,
    AtUri,
    firehose_models,
    models,
    parse_subscribe_repos_message,
)


logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)
# envars

SURREAL_URI = os.getenv("SURREAL_URI", "ws://localhost:8808")

SURREAL_USERNAME = os.getenv("SURREAL_USERNAME", "root")
SURREAL_PASSWORD = os.getenv("SURREAL_PASSWORD", "root")
SURREAL_NAMESPACE = os.getenv("SURREAL_NAMESPACE", "bsky")
SURREAL_DATABASE = os.getenv("SURREAL_DATABASE", "bsky")

# surrealdb uri
db = AsyncSurrealDB(SURREAL_URI)


_INTERESTED_RECORDS = {
    # models.ids.AppBskyFeedLike: models.AppBskyFeedLike,
    models.ids.AppBskyFeedPost: models.AppBskyFeedPost,
    # models.ids.AppBskyGraphFollow: models.AppBskyGraphFollow,
}


def _get_ops_by_type(commit: models.ComAtprotoSyncSubscribeRepos.Commit) -> defaultdict:
    operation_by_type = defaultdict(lambda: {"created": [], "deleted": []})

    car = CAR.from_bytes(commit.blocks)
    for op in commit.ops:
        if op.action == "update":
            # not supported yet
            continue

        uri = AtUri.from_str(f"at://{commit.repo}/{op.path}")

        if op.action == "create":
            if not op.cid:
                continue

            create_info = {"uri": str(uri), "cid": str(op.cid), "author": commit.repo}

            record_raw_data = car.blocks.get(op.cid)
            if not record_raw_data:
                continue

            record = models.get_or_create(record_raw_data, strict=False)
            record_type = _INTERESTED_RECORDS.get(uri.collection)
            if record_type and models.is_record_type(record, record_type):
                operation_by_type[uri.collection]["created"].append(
                    {"record": record, **create_info}
                )

        if op.action == "delete":
            operation_by_type[uri.collection]["deleted"].append({"uri": str(uri)})

    return operation_by_type


def measure_events_per_second(func: callable) -> callable:
    def wrapper(*args) -> Any:
        wrapper.calls += 1
        cur_time = time.time()

        if cur_time - wrapper.start_time >= 1:
            logger.info(f"NETWORK LOAD: {wrapper.calls} events/second")
            wrapper.start_time = cur_time
            wrapper.calls = 0

        return func(*args)

    wrapper.calls = 0
    wrapper.start_time = time.time()

    return wrapper


async def signal_handler(_: int, __: FrameType) -> None:
    print("Keyboard interrupt received. Stopping...")

    # Stop receiving new messages
    await client.stop()


def bsky_post_index(idx: str) -> str:
    return f"bsky_feed_post:⟨{idx}⟩"


def bsky_user_index(idx: str) -> str:
    return f"bsky_user:⟨{idx}⟩"


async def process_data(post: dict) -> None:
    author = post["author"]
    record = post["record"]
    post_id = post["uri"].split("/")[-1]

    # post_index = f"{author}/{post_id}"
    # print(post)
    post_index = post["cid"]

    reply = (
        {
            "parent": record.reply.parent.cid,
            "root": record.reply.root.cid,
        }
        if record.reply
        else None
    )

    labels = []

    if record.labels:
        for label in record.labels.values:
            labels.append(label.val)

    # print(f"Reply to: {reply}")

    # print(f"Post ID: {post_id}")

    # print(f"Post Index: {post_index}")
    # await db.select
    # Check if index already exists
    surreal_post_index = bsky_post_index(post_index)
    conflict_check = await db.query(
        f"SELECT * FROM bsky_feed_post WHERE id = {surreal_post_index}"
    )
    # print(a)

    if conflict_check[0]["result"]:
        logger.warning(f"Post already exists, skipping: {post_index}")
        return

    embed = {
        "images": [],
        "videos": [],
        "external": [],
        "record": [],
        "record_with_media": [],
    }

    # Okay, let's record some embeds
    if record.embed:
        if isinstance(record.embed, models.AppBskyEmbedImages.Main):
            for image in record.embed.images:
                embed["images"].append(image.model_computed_fields)

        if isinstance(record.embed, models.AppBskyEmbedVideo.Main):
            embed["videos"].append(record.embed.video.model_computed_fields)

        if isinstance(record.embed, models.AppBskyEmbedExternal.Main):
            embed["external"].append(record.embed.external.model_computed_fields)

        if isinstance(record.embed, models.AppBskyEmbedRecord.Main):
            embed["record"].append(
                {
                    "cid": record.embed.record.cid,
                    "fields": record.embed.record.model_computed_fields,
                }
            )

        if isinstance(record.embed, models.AppBskyEmbedRecordWithMedia.Main):
            embed["record_with_media"].append(
                {
                    "cid": record.embed.record.record.cid,
                    "fields": record.embed.model_computed_fields,
                }
            )

    # if record.text == "":
    #     logger.warning(f"Empty post, not adding: {post_index}")
    #     return

    await db.create(
        thing="bsky_feed_post",
        data={
            "id": post_index,
            "post_id": post_id,
            "author": author,
            "text": record.text,
            "created_at": record.created_at,
            "language": record.langs,
            "labels": labels,
            "reply": reply,
            "embed": embed,
            "tags": record.tags,
        },
    )

    # Check if user already exists
    surreal_user_index = bsky_user_index(author)
    user_conflict_check = await db.query(
        f"SELECT * FROM bsky_user WHERE id = {surreal_user_index}"
    )

    if not user_conflict_check[0]["result"]:
        await db.create("bsky_user", {"id": author})

    # Tag author
    author_index = bsky_user_index(author)
    await db.query(f"RELATE {author_index}->posted->{surreal_post_index}")

    # Let's tag replies as well!

    for record in embed["record"]:
        record_index = bsky_post_index(record)
        logger.debug(
            f"Linking post {surreal_post_index} to record {record_index} due to embed"
        )
        await db.query(f"RELATE {surreal_post_index}->quoted->{record_index}")

    for record_with_media in embed["record_with_media"]:
        record_with_media_index = bsky_post_index(record_with_media)
        logger.debug(
            f"Linking post {surreal_post_index} to record with media {record_with_media_index} due to embed"
        )
        await db.query(
            f"RELATE {surreal_post_index}->quoted_with_media->{record_with_media_index}"
        )

    if reply:
        og_post = bsky_post_index(post_index)
        parent = bsky_post_index(reply["parent"])
        root = bsky_post_index(reply["root"])

        # root_post_check = await db.query(f"SELECT * FROM bsky_feed_post WHERE id = {root}")
        # if root_post_check[0]["result"] == []:
        #     logger.warning(f"Root post not found, not linking: {root}")
        #     return

        # parent_post_check = await db.query(f"SELECT * FROM bsky_feed_post WHERE id = {parent}")
        # if parent_post_check[0]["result"] == []:
        #     logger.warning(f"Parent post not found, not linking: {parent}")
        #     return

        logger.debug(f"Linking post {og_post} to parent {parent} and root {root}")
        resparent = await db.query(f"RELATE {og_post}->reply->{parent}")
        # print(resparent)
        resroot = await db.query(f"RELATE {og_post}->reply_root->{root}")
        # print(resroot)

    pass


async def main(firehose_client: AsyncFirehoseSubscribeReposClient) -> None:
    await db.connect()
    await db.use(namespace=SURREAL_NAMESPACE, database=SURREAL_DATABASE)
    await db.sign_in(password=SURREAL_PASSWORD, username=SURREAL_USERNAME)

    @measure_events_per_second
    async def on_message_handler(message: firehose_models.MessageFrame) -> None:
        commit = parse_subscribe_repos_message(message)
        if not isinstance(commit, models.ComAtprotoSyncSubscribeRepos.Commit):
            return

        if commit.seq % 20 == 0:
            firehose_client.update_params(
                models.ComAtprotoSyncSubscribeRepos.Params(cursor=commit.seq)
            )

        if not commit.blocks:
            return

        ops = _get_ops_by_type(commit)
        for created_post in ops[models.ids.AppBskyFeedPost]["created"]:
            # print(created_post)
            # print(json.dumps(created_post))
            # cid = created_post["cid"]
            # author = created_post["author"]
            # record = created_post["record"]
            # post_id = created_post["uri"].split("/")[-1]

            # inlined_text = record.text.replace("\n", " ")
            # print(f"Post ID: {post_id}")
            # print(record)

            # format as dict

            # logger.debug(
            #     f"NEW POST [CREATED_AT={record.created_at}][AUTHOR={author}]: {inlined_text}"
            # )

            await process_data(created_post)

    await client.start(on_message_handler)


if __name__ == "__main__":
    signal.signal(
        signal.SIGINT, lambda _, __: asyncio.create_task(signal_handler(_, __))
    )

    start_cursor = None

    params = None
    if start_cursor is not None:
        params = models.ComAtprotoSyncSubscribeRepos.Params(cursor=start_cursor)

    client = AsyncFirehoseSubscribeReposClient(params)

    # use run() for a higher Python version
    asyncio.run(main(client))
