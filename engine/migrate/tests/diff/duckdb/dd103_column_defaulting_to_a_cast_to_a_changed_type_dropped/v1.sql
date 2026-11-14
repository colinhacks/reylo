create type mood as enum ('a', 'b'); create table t (id integer, k mood default 'a'::mood, j varchar default 'b'::mood);
