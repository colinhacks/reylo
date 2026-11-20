create table p (id int, c text[]);
create table c (c text[] default '{}') inherits (p);
