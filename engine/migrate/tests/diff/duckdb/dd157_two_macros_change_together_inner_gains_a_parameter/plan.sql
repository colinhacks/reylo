create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro x;
drop macro y;
create macro y (a, b) as ((a > b));
create macro x (a) as (y(a, 0));
create table t (id integer, v integer, primary key (id), check (x(v)));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
