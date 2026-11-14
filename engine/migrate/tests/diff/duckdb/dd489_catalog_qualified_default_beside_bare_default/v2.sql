create macro m(x) as x + 2; create table t (id integer, a integer default m(1), b integer default memory.m(2), c integer default memory.main.m(3));
