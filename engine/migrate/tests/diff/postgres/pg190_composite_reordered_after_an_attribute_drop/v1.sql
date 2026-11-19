create type e as enum ('a');
create type item as (gone e, a int, b int);
create table t (id int primary key, v item);
