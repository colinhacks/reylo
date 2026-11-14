create type mood as enum ('a', 'b', 'c'); create macro as_mood(s) as s::mood; create table t (id integer primary key, s varchar, m varchar generated always as (as_mood(s)::varchar) virtual);
