-- loses data: c
drop table c;
create temporary table reylo_copy_p as select * from p;
drop table p;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table p (id integer, m varchar, primary key (id), check ((cast(m as mood) != 'b')));
insert into p (id, m) select id, m from temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_p;
