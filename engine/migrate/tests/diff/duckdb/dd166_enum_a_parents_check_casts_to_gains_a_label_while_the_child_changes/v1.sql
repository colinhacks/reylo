create type mood as enum ('a', 'b'); create table p (id integer primary key, m varchar, check (m::mood <> 'b')); create table c (id integer primary key, p_id integer references p (id));
