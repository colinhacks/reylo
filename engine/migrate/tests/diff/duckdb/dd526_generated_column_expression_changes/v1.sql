create table t (id integer primary key, price integer, qty integer, total integer generated always as (price * qty) virtual);
