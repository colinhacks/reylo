create domain d as bigint;
create table p (id int, n d);
create table c () inherits (p);
