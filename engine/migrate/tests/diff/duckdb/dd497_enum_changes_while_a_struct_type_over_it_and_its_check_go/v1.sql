create type mood as enum ('a', 'b'); create type pair as struct(m mood); create table t (id integer, m varchar, check (m::mood <> 'b'), check (row(m)::pair is not null));
