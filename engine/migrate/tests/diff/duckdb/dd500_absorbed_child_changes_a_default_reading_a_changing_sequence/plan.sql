-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table c alter column d drop default;
drop sequence s;
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
create table p (id integer, a integer, primary key (id), check ((a > 0)));
create table c (id integer, pid integer, d bigint default (nextval('s') + 1), primary key (id), foreign key (pid) references p (id));
insert into p (id, a) select id, a from temp.main.reylo_copy_p;
insert into c (id, pid, d) select id, pid, d from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
