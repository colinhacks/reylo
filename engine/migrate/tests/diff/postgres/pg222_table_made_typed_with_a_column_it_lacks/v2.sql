create type ty as (id int, a int, n int);
create table t of ty (n with options default 5);
