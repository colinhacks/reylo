create type st as enum ('a', 'b');
create type mood as enum ('happy', 'sad');
create table t (id integer primary key, s st default 'a', m mood);
