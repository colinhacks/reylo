create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro x;
create macro x () as (2);
create table t (id bigint default nextval('s'), v integer, primary key (id), check ((v >= x())));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
