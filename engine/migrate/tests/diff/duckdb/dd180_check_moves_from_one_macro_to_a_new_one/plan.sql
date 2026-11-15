create macro y (a) as ((a >= 0));
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, v integer, primary key (id), check (y(v)));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro x;
