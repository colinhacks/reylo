create macro m(x) as x + 5; create table t (a integer default memory.m(1), b integer default m(2));
