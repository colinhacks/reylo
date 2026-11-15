create type mood as enum ('a', 'b'); create table t (id integer, m varchar check (m::mood is not null), d varchar default 'a'::mood);
