create table t (id bigint primary key, n bigint, twice integer generated always as (n * 2) virtual);
