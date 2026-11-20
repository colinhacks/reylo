create type ty as (id int, c int);
create sequence s;
create table t of ty (c with options default nextval('s'));
alter sequence s owned by t.c;
