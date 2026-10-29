-- What a test narrows beyond a plain column of a SELECT's WHERE: the columns of a `*`, the columns
-- an inner join's own ON tests, a write's WHERE for its RETURNING, a WITH query read twice, and a
-- recursive WITH query that reads `*` or has SEARCH or CYCLE.
create table users (id int8 primary key, name text not null, nickname text);
create table orders (id int8 primary key, note text, total numeric, user_id int8);
create table comments (id int8 primary key, parent_id int8, author_id int8, body text not null);
create table staff (id int8 primary key, manager_id int8, name text);
insert into users values (1, 'a', null), (2, 'b', 'bee');
insert into orders values (1, null, null, null), (2, 'n', 5, 1), (3, 'm', null, 99);
insert into comments values (1, null, null, 'x'), (2, 1, 1, 'y'), (3, 2, 99, 'z'), (4, 1, null, 'w');
insert into staff values (1, null, 'boss'), (2, 1, null), (3, 2, 'c');
