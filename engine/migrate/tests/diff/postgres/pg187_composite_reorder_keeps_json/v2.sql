create type item as (doc json, a integer, b text);
create table t (id integer primary key, v item);
