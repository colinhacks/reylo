create type mood as enum ('a', 'b'); create type pair as struct(m mood); create table t (id integer primary key, p pair);
