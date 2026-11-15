create type st as enum ('a', 'b', 'c'); create table t (id integer primary key, s st); create view v as select id, s::varchar as s from t where s = 'a'::st; create macro is_a(x) as x = 'a'::st;
