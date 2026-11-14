drop type mood;
create type mood as enum('a', 'b', 'c');
create temporary table reylo_copy_c as select * from c;
drop table c;
create table c (id integer, p_id integer, primary key (id));
insert into c (id, p_id) select id, p_id from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_c;
alter table p alter column m set data type mood;
