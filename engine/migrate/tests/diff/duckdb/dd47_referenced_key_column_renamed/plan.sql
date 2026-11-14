create temporary table reylo_copy_a as select * from a;
create temporary table reylo_copy_b as select * from b;
drop table b;
drop table a;
create table a (aid bigint, name varchar, primary key (aid));
create table b (id bigint, a_id bigint, foreign key (a_id) references a (aid));
insert into a (aid, name) select id, name from temp.main.reylo_copy_a;
insert into b (id, a_id) select id, a_id from temp.main.reylo_copy_b;
drop table temp.main.reylo_copy_a;
drop table temp.main.reylo_copy_b;
-- a.id is renamed aid, keeping its rows. If they are two columns, drop the one and add the other instead.
