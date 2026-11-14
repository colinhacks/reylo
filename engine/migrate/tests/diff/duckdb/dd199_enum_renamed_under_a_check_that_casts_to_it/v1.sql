create type mood as enum ('a', 'b'); create table t (id integer primary key, m varchar check (m::mood is not null));
