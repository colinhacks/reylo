-- loses data: b.gone
create temporary table reylo_copy_b as select * from b;
drop table b;
create table b (id bigint, a_id bigint, foreign key (a_id) references a (id));
insert into b (id, a_id) select id, a_id from temp.main.reylo_copy_b;
drop table temp.main.reylo_copy_b;
