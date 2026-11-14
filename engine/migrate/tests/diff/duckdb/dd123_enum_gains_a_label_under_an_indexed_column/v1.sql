create type mood as enum ('a', 'b'); create table t (id integer primary key, m mood); create index t_m on t (m);
