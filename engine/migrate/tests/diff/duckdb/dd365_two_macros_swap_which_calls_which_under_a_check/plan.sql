create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m1;
drop macro m2;
create macro m2 () as (2);
create macro m1 () as ((m2() - 1));
create table t (id integer, n integer, primary key (id), check ((n < ((m1() + m2()) + 10))));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
