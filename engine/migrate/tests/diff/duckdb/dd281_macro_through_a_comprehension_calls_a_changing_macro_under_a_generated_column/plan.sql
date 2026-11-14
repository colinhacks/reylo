create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro clean;
create macro clean (s) as (lower("trim"(s)));
create table t (id integer, tags varchar[], cleaned varchar[] generated always as (clean_all(tags)) virtual, primary key (id));
insert into t (id, tags) select id, tags from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
