create schema b; create macro valid() as 1; create macro k(x) as x > -1; create table b.p2 (id integer, n integer check (k(n)), e integer default valid());
