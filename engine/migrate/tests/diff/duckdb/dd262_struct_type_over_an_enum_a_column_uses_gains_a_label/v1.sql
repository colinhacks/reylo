create type mood as enum ('a', 'b'); create type pair as struct(m mood, n integer); create table t (id integer primary key, p pair);
