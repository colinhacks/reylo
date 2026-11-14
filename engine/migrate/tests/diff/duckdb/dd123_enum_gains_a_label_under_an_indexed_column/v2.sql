create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, m mood); create index t_m on t (m);
