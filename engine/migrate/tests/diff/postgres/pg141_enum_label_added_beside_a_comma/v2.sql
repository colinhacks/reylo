create type tag as enum ('a', 'b, c', 'it''s', 'd, e');
create table t (id integer primary key, g tag);
