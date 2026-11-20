create domain d as int;
create table p (id int, e text, n d);
create table c (e text) inherits (p);
