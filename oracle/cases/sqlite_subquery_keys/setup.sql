-- A subquery, a WITH query or a view is pinned to one row by a unique key of its rows, as a table
-- is by its index: a key of a FROM item the select list keeps unchanged, every other item pinned
-- to one row per row of it, or the select list's GROUP BY or DISTINCT. SQLite reads a view as the
-- subquery it is. Anything that can repeat a row leaves no key.
create table users (id integer primary key, email text not null unique, name text not null collate nocase unique, deleted_at text);
create table posts (id integer primary key, author_id integer not null references users (id), title text not null);
create table tags (post_id integer not null references posts (id), tag text not null, primary key (post_id, tag));
create view public_posts as select id, title from posts;
create view renamed_posts (post_id, headline) as select id, title from posts;
create view post_authors as select p.id, p.title, u.name from posts p join users u on u.id = p.author_id;
create view author_counts as select author_id, count(*) as n from posts group by author_id;
create view recent_posts as select * from public_posts where id > 10;
create view post_tags as select p.id, t.tag from posts p join tags t on t.post_id = p.id;
