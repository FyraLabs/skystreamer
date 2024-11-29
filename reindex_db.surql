# Reindex the database and redo graph connections, removing duplicates
# and ensuring that all connections are correct.

# Delete all edge tables

DEFINE TABLE bsky_feed_post;



DEFINE TABLE bsky_post_view AS
SELECT count() AS view_count
FROM bsky_feed_post GROUP ALL;

FOR $post IN (SELECT * FROM bsky_feed_post) {
    
};
