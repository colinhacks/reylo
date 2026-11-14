create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, m mood, label varchar generated always as (m::varchar || '!') virtual);
