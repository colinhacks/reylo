create type feeling as enum ('a', 'b'); create table t (id integer primary key, m varchar check (m::feeling is not null));
