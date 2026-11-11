-- Edge cases of DuckDB's arrays of whole rows: names that resolve elsewhere, outer joins, set operations, grouping and windows inside the item, correlated expressions.
create table posts (id bigint primary key, title varchar not null);
create table comments (id bigint primary key, post_id bigint not null, body varchar, big bigint not null);
create table replies (id bigint primary key, comment_id bigint not null, note varchar not null);
create table holder (id integer primary key, c struct(n integer));
insert into posts values (1, 'has'), (2, 'none');
insert into comments values (1, 1, 'x', 5), (2, 1, null, 7);
insert into replies values (1, 1, 'r');
insert into holder values (1, {'n': null}), (2, null);
