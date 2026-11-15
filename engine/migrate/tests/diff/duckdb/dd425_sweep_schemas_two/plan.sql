create temporary table reylo_copy_p as select * from p;
drop table p;
drop type s.mood;
drop macro s.m2;
create macro s.m2 (x) as ((x + 2));
create type s.mood as enum('a', 'b', 'c');
create table p (id integer, v varchar, n2 integer default s.m2(1), primary key (id), check ((cast(v as s.mood) is not null)));
insert into p (id, v, n2) select id, v, n2 from temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_p;
