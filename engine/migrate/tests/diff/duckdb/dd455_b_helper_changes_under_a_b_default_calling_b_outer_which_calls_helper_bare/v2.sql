create schema b; create macro helper() as 1; create macro b.helper() as 20; create macro b.outer() as helper(); create table b.t (id integer primary key, n integer default b.outer());
