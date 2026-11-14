create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, m mood); create table c (id integer primary key, t_id integer references t (id));
