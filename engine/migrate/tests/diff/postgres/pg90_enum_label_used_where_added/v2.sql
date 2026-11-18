create type st as enum ('a', 'b', 'c');
create type mood as enum ('happy', 'sad', 'meh');
create table t (id integer primary key, s st default 'c', m mood, m2 mood default 'meh');
