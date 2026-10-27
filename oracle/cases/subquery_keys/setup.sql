-- A subquery, a WITH query or a view is pinned to one row by a unique key of its rows, as a table
-- is by its index: a key of a FROM item the select list keeps unchanged, every other item pinned
-- to one row per row of it, or the select list's GROUP BY or DISTINCT. Anything that can repeat a
-- row leaves no key.
create table users (id bigint primary key, email text not null unique, name text not null, deleted_at timestamptz);
create table posts (id bigint primary key, author_id bigint not null references users (id), title text not null);
create table tags (post_id bigint not null references posts (id), tag text not null, primary key (post_id, tag));
create unique index users_live_name on users (name) where deleted_at is null;
create schema archive;
create table archive.posts (id bigint primary key, title text not null);
create view public_posts as select id, title from posts;
create view renamed_posts (post_id, headline) as select id, title from posts;
create view post_authors as select p.id, p.title, u.name from posts p join users u on u.id = p.author_id;
create view author_counts as select author_id, count(*) as n from posts group by author_id;
create view recent_posts as select * from public_posts where id > 10;
create view archived as select id, title from archive.posts;
create view post_tags as select p.id, t.tag from posts p join tags t on t.post_id = p.id;
create unique index users_email_lower on users (lower(email));
