create schema b; create table t (id integer primary key, n integer); create table c (id integer primary key, t_id integer references t (id));
     create table b.t (id integer primary key, n integer check (n > 0)); create table b.c (id integer primary key, t_id integer references t (id));
