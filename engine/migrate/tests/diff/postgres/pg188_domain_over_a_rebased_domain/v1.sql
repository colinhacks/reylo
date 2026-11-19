create domain d as integer;
create domain child as d;
create table t (id integer primary key, c child, e d);
