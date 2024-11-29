import base64
import json
from typing import Any
import surrealdb as sur
import time
import logging

db = sur.SurrealDB("ws://localhost:8000")
db.connect()
db.use("bsky", "bsky")

CHUNK_SIZE = 1000

# You could query the actual number from the database but we're hardcoding it because looking for it times out
# the connection
total: int = 5018627

index: int = 0
# We're gonna keep streaming in chunks until we reach the total using select query

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


def measure_lines_per_second(func: callable) -> callable:
    def wrapper(*args, **kwargs) -> Any:
        wrapper.calls += 1
        cur_time = time.time()

        if cur_time - wrapper.start_time >= 1:
            logger.info(f"LINES PROCESSED: {wrapper.calls} lines/second")
            wrapper.start_time = cur_time
            wrapper.calls = 0

        return func(*args, **kwargs)

    wrapper.calls = 0
    wrapper.start_time = time.time()

    return wrapper


@measure_lines_per_second
def write_line_to_file(file, line: str) -> None:
    file.write(line)


class FunnyEncoder(json.JSONEncoder):
    def default(self, o):
        if isinstance(o, bytes):
            return base64.b64encode(o).decode("utf-8")
        elif isinstance(o, sur.RecordID):
            return str(o.id)
        elif isinstance(o, str):
            return o.encode("utf-8").decode("utf-8")
        else:
            return super().default(o)


with open("data.jsonl", "w") as f:
    while index < total:
        # logger.info(f"Index: {index}")
        # Selecting the data from the table
        data = db.query(
            f"SELECT * FROM bsky_feed_post LIMIT {CHUNK_SIZE} START {index} TEMPFILES"
        )

        # set index
        # index += CHUNK_SIZE

        rows = data[0]["result"]
        # exit()
        # print(f"Rows: {len(rows)}")
        # print(rows)

        for row in rows:
            # replace 'id' with the record_id string

            # row["id"] = str(row["id"].id)
            # exit()
            write_line_to_file(f, json.dumps(row, cls=FunnyEncoder, ensure_ascii=False) + "\n")

        # print(len(data))

        index += CHUNK_SIZE


# q = db.query("SELECT count() FROM bsky_feed_post GROUP BY count")

# print(q)
#
