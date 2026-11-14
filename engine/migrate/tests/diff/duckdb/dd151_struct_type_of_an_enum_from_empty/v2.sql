create type mood as enum ('a', 'b'); create type a_rec as struct(m mood, n integer); create table t (id integer primary key, r a_rec);
