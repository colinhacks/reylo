create table p (id int, c text default 'z');
create table c (c text default 'b') inherits (p);
