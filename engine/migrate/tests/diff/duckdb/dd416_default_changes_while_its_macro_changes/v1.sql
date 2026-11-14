create macro m(x) as x + 1; create table t (a integer default m(1), b integer);
