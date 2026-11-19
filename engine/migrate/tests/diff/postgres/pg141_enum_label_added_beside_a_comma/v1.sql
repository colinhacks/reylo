create type tag as enum ('a', 'b, c', 'it''s');
create table t (id integer primary key, g tag);
