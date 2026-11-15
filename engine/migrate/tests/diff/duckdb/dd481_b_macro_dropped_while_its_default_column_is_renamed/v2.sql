create schema b; create macro valid() as 1; create table b.p2 (id integer, e integer default valid());
