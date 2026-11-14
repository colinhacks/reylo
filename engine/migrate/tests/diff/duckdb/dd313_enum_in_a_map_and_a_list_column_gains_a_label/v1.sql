create type mood as enum ('a', 'b'); create table t (id integer primary key, mp map(varchar, mood), l mood[]);
