create type st as enum ('a', 'b');
create table t (id integer primary key, s st default 'a');
