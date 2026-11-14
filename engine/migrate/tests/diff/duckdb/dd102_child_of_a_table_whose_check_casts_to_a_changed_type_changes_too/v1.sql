create type mood as enum ('a', 'b'); create table p (id integer primary key, v varchar check (v::mood is not null)); create table c (id integer, p_id integer references p (id));
