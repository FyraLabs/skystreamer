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
    AsyncClient,
    parse_subscribe_repos_message,
)

async_bsky_client = AsyncClient()

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)
# envars

SURREAL_URI = os.getenv("SURREAL_URI", "ws://localhost:8808")

SURREAL_USERNAME = os.getenv("SURREAL_USERNAME", "root")
SURREAL_PASSWORD = os.getenv("SURREAL_PASSWORD", "root")
SURREAL_NAMESPACE = os.getenv("SURREAL_NAMESPACE", "bsky")
SURREAL_DATABASE = os.getenv("SURREAL_DATABASE", "bsky")

DOWNLOAD_BLOBS = os.getenv("DOWNLOAD_BLOBS", False)

# surrealdb uri
db = AsyncSurrealDB(SURREAL_URI)


_INTERESTED_RECORDS = {
    # models.ids.AppBskyFeedLike: models.AppBskyFeedLike,
    models.ids.AppBskyFeedPost: models.AppBskyFeedPost,
    # models.ids.AppBskyGraphFollow: models.AppBskyGraphFollow,
}


async def download_blob(did: str, cid: str) -> bytes:
    blob = await async_bsky_client.com.atproto.sync.get_blob({"did": did, "cid": cid})
    return blob


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


def measure_posts_per_second(func: callable) -> callable:
    def wrapper(*args) -> Any:
        wrapper.calls += 1
        cur_time = time.time()

        if cur_time - wrapper.start_time >= 1:
            logger.info(f"POSTS PROCESSED: {wrapper.calls} posts/second")
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


@measure_posts_per_second
async def process_data(post: dict) -> None:
    logger.debug(f"Processing post: {post['uri']}")
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
    # Check if index already exists
    surreal_post_index = bsky_post_index(post_index)
    # conflict_check = await db.query(
    #     f"SELECT * FROM bsky_feed_post WHERE id = {surreal_post_index}"
    # )

    # if conflict_check[0]["result"]:
    #     logger.warning(f"Post already exists, skipping: {post_index}")
    #     return

    embed = {
        "images": [],
        "videos": [],
        "external": [],
        "record": [],
    }

    # Okay, let's record some embeds
    if record.embed:
        if isinstance(record.embed, models.AppBskyEmbedImages.Main):
            for image in record.embed.images:
                # print(image.image.model_dump())
                image_data = {
                    "blob_ref": image.image.model_dump(),
                    "cid": image.image.cid.encode(),
                }
                if DOWNLOAD_BLOBS:
                    image_data["data"] = await download_blob(author, image.image.cid)
                embed["images"].append(image_data)

        if isinstance(record.embed, models.AppBskyEmbedVideo.Main):
            video_data = {
                "blob_ref": record.embed.video.model_dump(),
                "cid": record.embed.video.cid.encode(),
            }
            if DOWNLOAD_BLOBS:
                video_data["data"] = await download_blob(author, record.embed.video.cid)
            embed["videos"].append(video_data)

        if isinstance(record.embed, models.AppBskyEmbedExternal.Main):
            embed["external"].append(record.embed.external.model_dump())

        if isinstance(record.embed, models.AppBskyEmbedRecord.Main):
            embed["record"].append(
                {
                    "cid": record.embed.record.cid,
                    "fields": record.embed.record.model_computed_fields,
                }
            )
            # print(record.embed.record.model_dump_json())

        if isinstance(record.embed, models.AppBskyEmbedRecordWithMedia.Main):
            # a_media = []
            for image in record.embed.media:
                # atproto_client.models.app.bsky.embed.images.Main | atproto_client.models.app.bsky.embed.video.Main | atproto_client.models.app.bsky.embed.external.Main
                if isinstance(image, models.AppBskyEmbedImages.Main):
                    for img in image.images:
                        image_data = {
                            "blob_ref": img.image.model_dump(),
                            "cid": img.image.cid.encode(),
                        }
                        if DOWNLOAD_BLOBS:
                            image_data["data"] = await download_blob(author, img.image.cid)
                        embed["images"].append(image_data)
                if isinstance(image, models.AppBskyEmbedVideo.Main):
                    video_data = {
                        "blob_ref": image.video.model_dump(),
                        "cid": image.video.cid.encode(),
                    }
                    if DOWNLOAD_BLOBS:
                        video_data["data"] = await download_blob(author, image.video.cid)
                    embed["videos"].append(video_data)
                if isinstance(image, models.AppBskyEmbedExternal.Main):
                    embed["external"].append(image.external.model_dump())
            embed["record"].append(
                {
                    "cid": record.embed.record.record.cid,
                    "fields": record.embed.model_dump(),
                }
            )

    # print(embed)

    # if record.text == "":
    #     logger.warning(f"Empty post, not adding: {post_index}")
    #     return
    while True:
        try:
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
                    "images": embed["images"],
                    "videos": embed["videos"],
                    "quotes": embed["record"],
                    "external_links": embed["external"],
                    "tags": record.tags,
                },
            )
            break
        except Exception as e:
            if "already exists" in str(e):
                return
            elif "Resource busy" in str(e):
                logger.warning("Resource busy, retrying...")
                await asyncio.sleep(0.2)
            else:
                logger.error(f"Error creating post: {e}")
                return

    # Check if user already exists
    while True:
        try:
            await db.create("bsky_user", {"id": author})
            break
        except Exception as e:
            if "already exists" in str(e):
                return
            elif "Resource busy" in str(e):
                logger.warning("Resource busy, retrying...")
                await asyncio.sleep(0.2)
            else:
                logger.error(f"Error creating user: {e}")
                return

    # Tag author
    author_index = bsky_user_index(author)
    await db.query(f"RELATE {author_index}->posted->{surreal_post_index}")
    # Let's tag replies as well!

    for record in embed["record"]:
        record_index = bsky_post_index(record["cid"])
        logger.debug(
            f"Linking post {surreal_post_index} to record {record_index} due to embed"
        )
        await db.query(f"RELATE {surreal_post_index}->quoted->{record_index}")

    if reply:
        og_post = bsky_post_index(post_index)
        parent = bsky_post_index(reply["parent"])
        root = bsky_post_index(reply["root"])
        logger.debug(f"Linking post {og_post} to parent {parent} and root {root}")
        await db.query(f"RELATE {og_post}->reply->{parent}")
        # print(resparent)
        await db.query(f"RELATE {og_post}->reply_root->{root}")
        # print(resroot)

    pass


#executor = ThreadPoolExecutor(max_workers=6)

async def main(firehose_client: AsyncFirehoseSubscribeReposClient) -> None:
    await db.connect()
    await db.use(namespace=SURREAL_NAMESPACE, database=SURREAL_DATABASE)
    await db.sign_in(password=SURREAL_PASSWORD, username=SURREAL_USERNAME)

    queue = asyncio.Queue()

    @measure_events_per_second
    async def on_message_handler(message: firehose_models.MessageFrame) -> None:
        # For each message we get we parse it
        commit = parse_subscribe_repos_message(message)
        # We only accept commits
        if not isinstance(commit, models.ComAtprotoSyncSubscribeRepos.Commit):
            return

        # Update the cursor every 20 commits
        if commit.seq % 20 == 0:
            firehose_client.update_params(
                models.ComAtprotoSyncSubscribeRepos.Params(cursor=commit.seq)
            )

        # We only care about commits with blocks
        if not commit.blocks:
            return

        # Now, check the ops in the commit
        ops = _get_ops_by_type(commit)
        # For each bsky post created in the commit, send it in the queue
        for created_post in ops[models.ids.AppBskyFeedPost]["created"]:
            await queue.put(created_post)

    async def worker() -> None:
        while True:
            post = await queue.get()
            try:
                # logger.info(f"Worker {asyncio.current_task().get_name()} processing post: {post['uri']}")
                await process_data(post)
            finally:
                queue.task_done()

    # General worker pool stuff

    workers = [asyncio.create_task(worker()) for _ in range(6)]
    loop = asyncio.get_running_loop()
    async def shutdown() -> None:
        await client.stop()
        for worker_task in workers:
            worker_task.cancel()
        await db.close()
        loop.stop()

    loop.add_signal_handler(signal.SIGINT, lambda: asyncio.create_task(shutdown()))

    await client.start(on_message_handler)
    await queue.join()

    for worker_task in workers:
        try:
            await worker_task
        except asyncio.CancelledError:
            pass

if __name__ == "__main__":
    signal.signal(
        signal.SIGINT, lambda _, __: asyncio.create_task(signal_handler(_, __))
    )

    start_cursor = None

    params = None
    if start_cursor is not None:
        params = models.ComAtprotoSyncSubscribeRepos.Params(cursor=start_cursor)

    client = AsyncFirehoseSubscribeReposClient(params)

    asyncio.run(main(client))
