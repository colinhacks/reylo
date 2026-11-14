create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, name varchar, meta struct(a integer), tags varchar[], up varchar generated always as (upper("name")) virtual, ma integer generated always as (struct_extract(meta, 'a')) virtual, first varchar generated always as (tags[1]) virtual, sized varchar generated always as (case  when ((len(tags) > 1)) then ('many') else 'few' end) virtual, txt varchar generated always as (((cast(id as varchar) || '-') || "name")) virtual, primary key (id), check (("name" != '')));
insert into t (id, name, meta, tags) select id, name, meta, tags from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
