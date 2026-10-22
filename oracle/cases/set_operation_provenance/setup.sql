-- A set operation's columns trace to its leftmost arm, so a column read through one is as
-- nullable as its arms together say. A column the statement reads straight from one of its own
-- tables keeps its table's NOT NULL however many set operations its WITH queries and subqueries
-- hold, and so does each arm of a union read as a WITH query.
create table users (id bigint primary key, name text not null, email text);
create table posts (id bigint primary key, user_id bigint not null references users, title text not null, published_at timestamptz);
create table comments (id bigint primary key, post_id bigint not null references posts, body text not null);
create table categories (id bigint primary key, parent_id bigint references categories, name text not null);
create table events (id bigint primary key, kind text not null check (kind in ('click', 'key')));
insert into events values (1, 'click'), (2, 'key');
