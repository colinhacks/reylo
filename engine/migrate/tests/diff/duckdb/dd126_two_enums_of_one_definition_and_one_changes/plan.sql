drop type b;
create type b as enum('x', 'y', 'z');
alter table t alter column cb set data type b;
