create domain d as bigint;
create type p as (a d, b text);
create table t (id integer primary key, c p);
