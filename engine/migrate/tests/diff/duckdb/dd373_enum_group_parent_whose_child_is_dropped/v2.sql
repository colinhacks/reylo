create type mood as enum ('a', 'b', 'c'); create table p (id integer primary key, m varchar check (m::mood <> 'b'));
