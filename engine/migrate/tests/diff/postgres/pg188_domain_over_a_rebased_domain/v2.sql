create domain d as bigint;
create domain child as d default 1;
create table t (id integer primary key, c child, e d);
