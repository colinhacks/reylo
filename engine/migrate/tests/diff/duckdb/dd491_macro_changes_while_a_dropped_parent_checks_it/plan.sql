-- loses data: x
create temporary table reylo_copy_y as select * from y;
drop table y;
drop table x;
drop macro m;
create macro m (x) as ((x > -1));
create table y (id integer, xid integer, e integer, primary key (id), check (m(e)));
insert into y (id, xid, e) select id, xid, e from temp.main.reylo_copy_y;
drop table temp.main.reylo_copy_y;
