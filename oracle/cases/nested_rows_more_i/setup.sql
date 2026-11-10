-- Edge cases of the check that a LATERAL item reads the same in place as alone: an outer column of the very row type, names inside nested subqueries, set operations, joins under an alias, functions, WITH queries, and items that only describe in place.
create type addr as (street text, zip int);
create table users (id bigint primary key, name text not null);
create table posts (id bigint primary key, author_id bigint not null references users, title text not null);
create table comments (id bigint primary key, post_id bigint not null references posts, body text, big bigint not null);
create table replies (id bigint primary key, comment_id bigint not null references comments, body text not null);
create table cc (id int primary key, c addr);
insert into users values (1, 'ada'), (2, 'bob');
insert into posts values (1, 1, 'has'), (2, 1, 'none');
insert into comments values (1, 1, 'x', 5), (2, 1, null, 7);
insert into replies values (1, 1, 'r');
insert into cc values (1, row('s', 1)), (2, null);
create table holder2 (id int primary key, c replies);
insert into holder2 values (1, null), (2, row(null, null, null)), (3, row(1, 1, 'r'));
create function comments_for(p bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments where post_id = p $$;
