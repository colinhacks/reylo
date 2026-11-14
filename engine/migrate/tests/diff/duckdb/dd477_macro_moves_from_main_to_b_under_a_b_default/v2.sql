create schema b; create macro b.valid() as 1; create table b.p2 (id integer, d integer default valid());
