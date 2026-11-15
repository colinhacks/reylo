create type mood as enum ('a', 'b', 'c'); create type pair as struct(x mood); create table t (id integer primary key, m mood);
