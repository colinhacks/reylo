create type mood as enum ('a', 'b'); create macro default_mood() as 'a'::mood; create table t (id integer primary key, m varchar default default_mood());
