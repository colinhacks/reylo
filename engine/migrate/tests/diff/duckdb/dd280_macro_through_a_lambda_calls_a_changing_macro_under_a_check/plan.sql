create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t (id integer, tags varchar[], ok boolean generated always as (all_valid(tags)) virtual, primary key (id));
insert into t (id, tags) select id, tags from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
