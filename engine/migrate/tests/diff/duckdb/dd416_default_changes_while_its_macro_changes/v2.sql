create macro m(x) as x + 2; create table t (a integer default m(2), b integer);
