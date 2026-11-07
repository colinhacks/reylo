-- A temporary table of the session that shadows a table of the same name is the one an
-- unqualified name reads, so the table's key and NOT NULL columns say nothing of it.
create table comments (id bigint primary key, post_id bigint not null, body varchar not null);
insert into comments values (1, 1, 'a'), (2, 1, 'b');
create temp table comments (id bigint, post_id bigint, body varchar);
insert into temp.comments values (1, 1, null), (1, 1, null);
