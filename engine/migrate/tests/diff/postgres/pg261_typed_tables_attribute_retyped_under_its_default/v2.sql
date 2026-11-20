create type ty as (id int, a int);
create table t of ty (a with options default 1);
