create type mood as enum ('a', 'b', 'c'); create table p (id integer primary key, m varchar default 'a'::mood, n integer check (n > 0)); create table c (id integer primary key, p_id integer);
