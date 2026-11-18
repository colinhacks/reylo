create type st as enum ('a', 'b', 'c');
create table t (id integer primary key, s st default 'c');
