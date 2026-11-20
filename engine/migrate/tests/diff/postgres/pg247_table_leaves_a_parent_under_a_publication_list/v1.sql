create table p (id int not null);
create table h () inherits (p);
create publication pub for table only h (id);
