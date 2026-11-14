-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table c alter column id drop default;
drop sequence s;
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
alter table c alter column id set default nextval('s');
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
create table p (id integer, n integer, primary key (id), check ((n > 0)));
create table c (id bigint default nextval('s'), p_id integer, note varchar, foreign key (p_id) references p (id));
insert into p (id, n) select id, n from temp.main.reylo_copy_p;
insert into c (id, p_id) select id, p_id from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
