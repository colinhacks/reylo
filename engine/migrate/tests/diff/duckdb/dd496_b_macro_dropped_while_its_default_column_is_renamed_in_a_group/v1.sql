create schema b; create macro valid() as 1; create macro b.valid() as 2; create macro k(x) as x > 0; create table b.p2 (id integer, n integer check (k(n)), d integer default valid());
