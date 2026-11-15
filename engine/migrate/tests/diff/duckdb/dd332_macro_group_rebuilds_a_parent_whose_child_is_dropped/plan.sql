-- loses data: c
drop table c;
create temporary table reylo_copy_p as select * from p;
drop table p;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table p (id integer, code varchar, primary key (id), check ("valid"(code)));
insert into p (id, code) select id, code from temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_p;
