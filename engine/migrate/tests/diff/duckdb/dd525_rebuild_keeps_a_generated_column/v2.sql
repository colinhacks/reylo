create table t (id integer primary key, price integer check (price >= 0), qty integer, total integer generated always as (price * qty) virtual);
