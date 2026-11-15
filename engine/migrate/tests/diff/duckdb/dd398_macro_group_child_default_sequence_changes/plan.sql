-- DuckDB makes seq again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
drop sequence seq;
drop macro m;
create macro m (x) as ((x > -1));
create sequence seq start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
create table p (id integer, primary key (id), check (m(id)));
create table c (id bigint default nextval('seq'), p integer, foreign key (p) references p (id));
insert into p (id) select id from temp.main.reylo_copy_p;
insert into c (id, p) select id, p from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
