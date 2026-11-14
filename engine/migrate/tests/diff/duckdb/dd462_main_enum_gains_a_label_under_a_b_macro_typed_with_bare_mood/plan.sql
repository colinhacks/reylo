drop type mood;
create type mood as enum('m', 'n');
create temporary table reylo_copy_t as select * from b.t;
drop table b.t;
drop macro b.f;
create macro b.f (x enum('m', 'n')) as (cast(x as VARCHAR));
create table b.t (id integer, s varchar, primary key (id), check ((b.f(s) is not null)));
insert into b.t (id, s) select id, s from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
