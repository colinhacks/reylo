-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_t as select * from t;
drop table t;
drop sequence s;
drop macro x;
create macro x () as (2);
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
create table t (id bigint default nextval('s'), v integer, primary key (id), check ((v >= x())));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
comment on sequence s is 'ids, by two';
