-- MySQL's multi-table DELETE and UPDATE, which return no rows (found by sqlc's corpus on
-- 2026-10-11, where `delete jt.*, pt.* from …`, which the engine's parser did not read then, was
-- `many` with no columns).
create table users (id int primary key, name varchar(20) not null);
create table emails (id int primary key, user_id int not null, email varchar(40) not null);
