create temporary table reylo_copy_t as select * from a.t;
drop table a.t;
drop macro a.valid;
create macro a."valid" (s) as ((length(s) > 0));
create table a.t (id integer, code varchar, primary key (id), check (a."valid"(code)));
insert into a.t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
