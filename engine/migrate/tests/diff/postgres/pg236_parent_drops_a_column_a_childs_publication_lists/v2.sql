create table p (id int);
create table h () inherits (p);
create publication pub for table h (id);
