create type tag as enum ('d|e', 'a', 'b, c', 'f');
create table t (id integer primary key, g tag);
