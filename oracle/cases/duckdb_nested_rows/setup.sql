-- An array of a subquery's whole rows: DuckDB's own describe names the fields, and the engine reads what may be NULL.
create table posts (id bigint primary key, title varchar not null);
create table comments (id bigint primary key, post_id bigint not null, body varchar, big hugeint not null);
create table replies (id bigint primary key, comment_id bigint not null, note varchar not null);
insert into posts values (1, 'has'), (2, 'none');
insert into comments values (1, 1, 'he said "hi"', 9007199254740993), (2, 1, null, 0);
insert into replies values (1, 1, 'r');
