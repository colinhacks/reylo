create type mood as enum ('a', 'b'); create table t (id integer primary key, m mood, label varchar generated always as (m::varchar || '!') virtual);
