create type ty as (id int, a text);
create table t of ty (a with options default '1'::text);
