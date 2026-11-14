create macro m(x) as x + 2; create table t (id integer, a integer default m(1), b integer, check (b.m() > 0));
