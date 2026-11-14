create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, s varchar, l varchar[] generated always as (cast(string_split(s, ',') as VARCHAR[])) virtual, n integer, primary key (id), check ((n > 0)));
insert into t (id, s) select id, s from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
