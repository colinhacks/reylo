create type mood as enum ('a', 'b'); create macro as_mood(s) as s::mood; create table t (id integer primary key, s varchar); create index t_m on t (as_mood(s));
