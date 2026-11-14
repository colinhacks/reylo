create type mood as enum ('a', 'b', 'c'); create table p (id integer primary key); create table t (id integer primary key, m mood, p integer references p (id));
