CREATE TABLE users (id UInt64, name String, email Nullable(String), age UInt8, balance Decimal(18, 4), created DateTime64(3, 'UTC'), role Enum8('admin' = 1, 'member' = 2), tags Array(String), plan LowCardinality(String)) ENGINE = MergeTree ORDER BY id;
CREATE TABLE orders (id UInt64, user_id UInt64, total Decimal(18, 2), note Nullable(String), placed Date) ENGINE = MergeTree ORDER BY id;
CREATE VIEW counted AS SELECT count() AS n FROM users GROUP BY ();
INSERT INTO users VALUES (1, 'ada', NULL, 36, 1.5, '2026-01-02 03:04:05.678', 'admin', ['a', 'b'], 'pro'), (2, 'bob', 'bob@example.com', 41, 2.25, '2026-01-03 00:00:00', 'member', [], 'free'), (2, 'twin', 'twin@example.com', 0, 0, '1970-01-01 00:00:00', 'member', ['c'], 'free');
INSERT INTO orders VALUES (10, 1, 9.99, 'first', '2026-01-02'), (11, 1, 0, NULL, '2026-01-03'), (12, 9, 5, 'orphan', '1970-01-01')
