create table p (id int, d text);
create table c (x int) inherits (p);
alter table c alter column d set not null;
