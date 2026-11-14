create table t (id bigint primary key, n integer, twice integer generated always as (n * 2) virtual);
