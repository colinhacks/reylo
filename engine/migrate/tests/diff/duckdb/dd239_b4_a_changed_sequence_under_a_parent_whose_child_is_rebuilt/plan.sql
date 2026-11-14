-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table p alter column id drop default;
drop sequence s;
create sequence s start with 1 increment by 5 minvalue 1 maxvalue 9223372036854775807;
alter table p alter column id set default nextval('s');
create temporary table reylo_copy_c as select * from c;
drop table c;
create table c (id integer, p_id bigint, v integer, primary key (id), foreign key (p_id) references p (id), check ((v > 0)));
insert into c (id, p_id, v) select id, p_id, v from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_c;
