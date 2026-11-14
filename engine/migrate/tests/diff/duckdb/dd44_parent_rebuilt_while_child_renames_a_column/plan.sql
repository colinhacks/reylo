create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
create table p (id bigint, n integer, primary key (id), check ((n > 0)));
create table c (id bigint, p_id bigint, memo varchar, primary key (id), foreign key (p_id) references p (id));
insert into p (id, n) select id, n from temp.main.reylo_copy_p;
insert into c (id, p_id, memo) select id, p_id, note from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
-- c.note is renamed memo, keeping its rows. If they are two columns, drop the one and add the other instead.
