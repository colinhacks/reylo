create domain d as integer;
create type p as (a d, b text);
create table t (id integer primary key, c p);
