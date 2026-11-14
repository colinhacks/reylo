create macro m(x) as x + 2; create table t (a integer default m(1), b integer, check (b.m() > 0));
