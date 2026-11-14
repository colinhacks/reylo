create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, n integer, primary key (id), check ((n > 0)));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro table top;
create macro top (k) as table (select id, n from t order by n desc limit k);
