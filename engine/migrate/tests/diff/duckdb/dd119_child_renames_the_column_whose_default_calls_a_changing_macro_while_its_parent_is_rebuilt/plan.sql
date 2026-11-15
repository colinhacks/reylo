alter table c alter column code drop default;
drop macro gen;
create macro gen () as ('g2');
alter table c alter column code set default gen();
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
create table p (id integer, name varchar, primary key (id), check (("name" != '')));
create table c (id integer, p_id integer, label varchar default gen(), primary key (id), foreign key (p_id) references p (id));
insert into p (id, name) select id, name from temp.main.reylo_copy_p;
insert into c (id, p_id, label) select id, p_id, code from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
-- c.code is renamed label, keeping its rows. If they are two columns, drop the one and add the other instead.
