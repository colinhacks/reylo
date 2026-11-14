-- loses data: x
-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_y as select * from y;
drop table y;
drop table x;
drop sequence s;
create sequence s start with 1 increment by 3 minvalue 1 maxvalue 9223372036854775807;
create table y (id integer, xid integer, e bigint default nextval('s'), primary key (id));
insert into y (id, xid, e) select id, xid, e from temp.main.reylo_copy_y;
drop table temp.main.reylo_copy_y;
