create domain d as integer;
create type p as (a d);
create table t (id integer primary key, c p);
