create type item as (a integer, doc json);
create table t (id integer primary key, v item);
