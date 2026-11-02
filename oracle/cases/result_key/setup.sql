-- The result key: the output columns that identify each row of a statement's result. A unique key
-- of a table is one where every column of it is in the output and none of them is NULL.
create table users (id bigint primary key, email text not null unique, handle text unique, name text, deleted_at timestamptz);
create unique index users_live_name on users (name) where deleted_at is null;
create table posts (id bigint primary key, author_id bigint not null references users (id), slug text not null, title text, unique (author_id, slug));
create table profiles (user_id bigint primary key references users (id), bio text);
create table tags (id int primary key, label text not null);
create view live_users as select id, email, name from users where deleted_at is null;
