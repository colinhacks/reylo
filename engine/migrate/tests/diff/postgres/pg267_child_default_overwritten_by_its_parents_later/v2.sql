create table p (id int, c text);
create table t (id int, c text default '7') inherits (p);
alter table p alter column c set default 'z';
