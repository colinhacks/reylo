create domain d as int;
create table p (id int, n d);
create table c () inherits (p);
