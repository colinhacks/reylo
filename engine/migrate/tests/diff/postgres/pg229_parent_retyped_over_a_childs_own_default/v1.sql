create table p (id int, c text);
create table c (c text default '7') inherits (p);
