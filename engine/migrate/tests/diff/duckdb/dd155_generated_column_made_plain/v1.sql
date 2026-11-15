create table t (id integer primary key, a integer, b integer, total integer generated always as (a + b) virtual, twice integer generated always as (a * 2) virtual, c integer check (c > 0));
