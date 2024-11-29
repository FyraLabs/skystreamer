
# Let's define our tables
DEFINE TABLE IF NOT EXISTS post TYPE NORMAL SCHEMALESS PERMISSIONS NONE;

-- ------------------------------
-- INDEXES
-- ------------------------------ 

DEFINE INDEX IF NOT EXISTS post_creation_date ON post FIELDS created_at;
DEFINE INDEX IF NOT EXISTS post_id ON post FIELDS id;

DEFINE TABLE IF NOT EXISTS post_count_view AS
SELECT count() AS post_count
FROM post GROUP ALL;

DEFINE TABLE IF NOT EXISTS post_by_language AS
SELECT count() AS post_count,
    language as language
FROM post GROUP BY language;

DEFINE TABLE IF NOT EXISTS post_by_user AS
SELECT count() AS post_count,
    author as author
FROM post GROUP BY author;