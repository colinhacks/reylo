-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_t as select * from t;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table t;
drop sequence s;
drop macro x;
create macro x () as (2);
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
create table t (id bigint default nextval('s'), v integer, primary key (id), check ((v >= x())));
create table c (id integer, t_id bigint, primary key (id));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
insert into c (id, t_id) select id, t_id from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_c;
