
# Let's define our tables
DEFINE TABLE post TYPE NORMAL SCHEMALESS PERMISSIONS NONE;

-- ------------------------------
-- INDEXES
-- ------------------------------ 

DEFINE INDEX post_creation_date ON post FIELDS created_at;
DEFINE INDEX post_id ON post FIELDS id;

DEFINE TABLE post_count_view AS
SELECT count() AS post_count
FROM post GROUP ALL;

DEFINE TABLE post_by_language AS
SELECT count() AS post_count,
    language as language
FROM post GROUP BY language;

DEFINE TABLE post_by_user AS
SELECT count() AS post_count,
    author as author
FROM post GROUP BY author;