create table p (id int, c boolean);
create table h () inherits (p);
create publication pub for table p (id, c);
