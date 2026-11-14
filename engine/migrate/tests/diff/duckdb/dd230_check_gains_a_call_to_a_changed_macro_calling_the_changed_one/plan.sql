create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro z;
drop macro y;
create macro y (a) as ((a > 1));
create macro z (a) as ((y(a) and (a < 50)));
create table t (id integer, v integer, primary key (id), check ((y(v) and z(v))));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
