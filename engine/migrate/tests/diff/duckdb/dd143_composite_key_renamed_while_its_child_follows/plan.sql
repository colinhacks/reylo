create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
create table p (a integer, bee integer, n integer, check ((n > 0)), primary key (a, bee));
create table c (id integer, pa integer, pb integer, primary key (id), foreign key (pa, pb) references p (a, bee));
insert into p (a, bee, n) select a, b, n from temp.main.reylo_copy_p;
insert into c (id, pa, pb) select id, pa, pb from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
-- p.b is renamed bee, keeping its rows. If they are two columns, drop the one and add the other instead.
