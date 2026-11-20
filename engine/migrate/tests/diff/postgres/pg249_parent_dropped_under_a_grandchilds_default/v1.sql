create sequence s;
create table t (id int default nextval('s') not null);
alter sequence s owned by t.id;
create table h () inherits (t);
create table g (id int) inherits (h);
