create table p (id int, c date);
create table h () inherits (p);
create publication pub for table h (id, c);
