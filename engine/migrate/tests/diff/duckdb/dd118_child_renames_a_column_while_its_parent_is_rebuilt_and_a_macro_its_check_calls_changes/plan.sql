create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
drop macro valid;
create macro "valid" (c) as ((length(c) > 0));
create table p (id integer, name varchar, primary key (id), check (("name" != '')));
create table c (id integer, p_id integer, code varchar, remark varchar, primary key (id), foreign key (p_id) references p (id), check ("valid"(code)));
insert into p (id, name) select id, name from temp.main.reylo_copy_p;
insert into c (id, p_id, code, remark) select id, p_id, code, note from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
-- c.note is renamed remark, keeping its rows. If they are two columns, drop the one and add the other instead.
