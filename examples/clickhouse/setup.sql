DROP TABLE IF EXISTS posts;
DROP TABLE IF EXISTS users;
CREATE TABLE users (
  id UInt32,
  name String,
  email Nullable(String),
  role Enum8('admin' = 1, 'member' = 2) DEFAULT 'member',
  created_at DateTime64(3, 'UTC') DEFAULT now64(3)
) ENGINE = MergeTree ORDER BY id;
CREATE TABLE posts (
  id UUID,
  user_id UInt32,
  title String,
  body Nullable(String),
  created_at DateTime64(3, 'UTC') DEFAULT now64(3)
) ENGINE = MergeTree ORDER BY (created_at, id);
INSERT INTO users (id, name, email, role) VALUES (1, 'ada', 'ada@example.com', 'admin'), (2, 'bob', NULL, 'member');
INSERT INTO posts (id, user_id, title, body, created_at) VALUES ('00000000-0000-4000-8000-000000000001', 1, 'Hello', 'first post', '2026-01-01 00:00:00'), ('00000000-0000-4000-8000-000000000002', 1, 'Again', NULL, '2026-01-02 00:00:00');
