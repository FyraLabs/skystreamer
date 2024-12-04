# Let's define our tables

BEGIN;

-------------------------------- TABLES --------------------------------

-- ------------------------------
-- USERS
DEFINE TABLE IF NOT EXISTS user TYPE NORMAL SCHEMALESS PERMISSIONS NONE;

-- ------------------------------
-- PRECOMPUTED VIEWS
-- ------------------------------
DEFINE TABLE IF NOT EXISTS user_count_total AS
SELECT count() AS user_count
FROM user GROUP ALL;



-- END USERS
-- ------------------------------


-- ------------------------------
-- POSTS
DEFINE TABLE IF NOT EXISTS post TYPE NORMAL SCHEMALESS PERMISSIONS NONE;

-- ------------------------------
-- FIELDS
-- ------------------------------ 

DEFINE FIELD IF NOT EXISTS created_at ON post FLEXIBLE TYPE datetime PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS author ON post FLEXIBLE TYPE record<user> PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS language ON post FLEXIBLE TYPE array<string> PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS text ON post FLEXIBLE TYPE string PERMISSIONS FULL;

-- ------------------------------
-- INDEXES
-- ------------------------------ 


DEFINE INDEX IF NOT EXISTS post_creation_date ON post FIELDS created_at;
DEFINE INDEX IF NOT EXISTS post_id ON post FIELDS id;


-- ------------------------------
-- PRECOMPUTED VIEWS
-- ------------------------------

DEFINE TABLE IF NOT EXISTS post_by_language AS
SELECT count() AS post_count,
    language as language
FROM post GROUP BY language;

DEFINE TABLE IF NOT EXISTS post_by_user AS
SELECT count() AS post_count,
    author as author
FROM post GROUP BY author;

DEFINE TABLE IF NOT EXISTS post_count_total AS
SELECT count() AS post_count
FROM post GROUP ALL;


-- END POSTS
-- ------------------------------


-- let's commit the transaction
COMMIT TRANSACTION;
-------------------------------- END TABLES --------------------------------

