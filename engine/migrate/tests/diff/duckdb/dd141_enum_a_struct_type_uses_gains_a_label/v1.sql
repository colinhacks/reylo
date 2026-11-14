create type mood as enum ('a', 'b'); create type rec as struct(m mood, n integer); create table t (id integer primary key, r rec);
