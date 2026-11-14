create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, seen varchar default cast(cast(now() as TIMESTAMP WITH TIME ZONE) as VARCHAR), n integer, primary key (id), check ((n > 0)));
insert into t (id, seen) select id, seen from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
