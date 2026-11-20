create type t as (a int, b text);
create table r (id int, c t);
create table z (c t, id int);
alter table z inherit r;
