create table p (id int, c text default 'a');
create table c (c text default 'b') inherits (p);
