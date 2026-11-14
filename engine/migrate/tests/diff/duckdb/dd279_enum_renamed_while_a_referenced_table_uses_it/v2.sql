create type feeling as enum ('a', 'b'); create table t (id integer primary key, m feeling); create table c (id integer primary key, t_id integer references t (id));
