create type mood as enum ('a', 'b', 'c'); create type rec as struct(m mood, n integer); create table t (id integer primary key, r rec);
