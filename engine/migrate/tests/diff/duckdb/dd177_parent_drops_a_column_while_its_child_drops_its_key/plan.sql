-- loses data: p.n
create temporary table reylo_copy_z as select * from z;
drop table z;
create table z (id integer, p_id integer, primary key (id));
insert into z (id, p_id) select id, p_id from temp.main.reylo_copy_z;
drop table temp.main.reylo_copy_z;
alter table p drop column n;
