-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table c alter column no drop default;
drop sequence s;
create sequence s start with 1 increment by 5 minvalue 1 maxvalue 9223372036854775807;
alter table c alter column no set default nextval('s');
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
create table p (id integer, name varchar, primary key (id), check (("name" != '')));
create table c (id integer, p_id integer, serial_no bigint default nextval('s'), primary key (id), foreign key (p_id) references p (id));
insert into p (id, name) select id, name from temp.main.reylo_copy_p;
insert into c (id, p_id, serial_no) select id, p_id, no from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
-- c.no is renamed serial_no, keeping its rows. If they are two columns, drop the one and add the other instead.
