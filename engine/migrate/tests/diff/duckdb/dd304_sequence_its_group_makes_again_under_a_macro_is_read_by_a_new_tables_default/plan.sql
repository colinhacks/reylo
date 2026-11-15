-- DuckDB makes zs again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_t as select * from t;
drop table t;
drop sequence zs;
drop macro a_lead;
create macro a_lead (s) as ((s != '-'));
create sequence zs start with 1 increment by 3 minvalue 1 maxvalue 9223372036854775807;
create table t (id bigint default nextval('zs'), code varchar, primary key (id), check (a_lead(code)));
insert into t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create table u (id bigint default nextval('zs'), n integer, primary key (id));
