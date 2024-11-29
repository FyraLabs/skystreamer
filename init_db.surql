
# Let's define our tables

DEFINE TABLE post;

DEFINE TABLE bsky_post_view AS
SELECT count() AS post_count
FROM post GROUP ALL;

DEFINE TABLE bsky_post_lang_view AS
SELECT count() AS post_count
FROM post GROUP BY language;