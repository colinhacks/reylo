create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro outer_m;
create macro outer_m () as (2);
create table t (id integer, n integer, primary key (id), check ((n < (outer_m() + 10))));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro inner_m;
