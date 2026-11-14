create temporary table reylo_copy_c as select * from c;
drop table c;
create table c (id integer, code varchar, primary key (id), foreign key (code) references "Parent Table" (code));
insert into c (id, code) select id, code from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_c;
