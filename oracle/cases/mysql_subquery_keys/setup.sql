-- A derived table, a WITH query or a view is pinned to one row by a unique key of its rows, as a
-- table is by its index: a key of a FROM item the select list keeps unchanged, every other item
-- pinned to one row per row of it, or the select list's GROUP BY or DISTINCT. Joins pin a table
-- through another table's key. Anything that can repeat a row leaves no key.
create table users (id bigint primary key, email varchar(200) not null unique, name varchar(200) not null, code varchar(20) unique);
create table posts (id bigint primary key, author_id bigint not null, title text not null);
create table tags (post_id bigint not null, tag varchar(100) not null, primary key (post_id, tag));
create view public_posts as select id, title from posts;
create algorithm = temptable view public_posts_temp as select id, title from posts;
create algorithm = merge view public_posts_merged as select id, title from posts;
create view renamed_posts (post_id, headline) as select id, title from posts;
create view post_authors as select p.id, p.title, u.name from posts p join users u on u.id = p.author_id;
create view author_counts as select author_id, count(*) as n from posts group by author_id;
create view recent_posts as select * from public_posts where id > 10;
create view post_tags as select p.id, t.tag from posts p join tags t on t.post_id = p.id;
create table scores (id bigint primary key, score double not null, rate double not null unique);
create view swapped (title, id) as select id, title from posts;
