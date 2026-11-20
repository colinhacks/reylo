create type p as (a int);
create table t (id int, e text[] default '{}', n p);
create table h (e text[] not null) inherits (t);
