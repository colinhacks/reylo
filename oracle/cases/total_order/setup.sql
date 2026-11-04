-- A total order: the ORDER BY's sort keys hold every column of a unique key of the rows it sorts,
-- so no two rows of a result tie and the order is the same on every run.
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
create table users (id bigint primary key, email text not null unique, handle text unique, name text, tag text collate ci not null unique);
create table posts (id bigint primary key, author_id bigint not null references users (id), title text, score int);
create table profiles (user_id bigint primary key references users (id), bio text);
create view live_posts as select id, author_id, title from posts where score > 0;
