create function f(int) returns int language sql immutable return $1 + 1;
create table p (a int, b text) partition by range (a);
create index pi on p (f(a));
create table child (a int, b text);
alter table p attach partition child for values from (0) to (10);
