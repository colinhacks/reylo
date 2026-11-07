-- Attacks on the inference of arrays of whole rows, fourth round: the fixes for outer names and padded nested arrays, under two LATERAL levels, columns of one name, and partly padded items.
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
