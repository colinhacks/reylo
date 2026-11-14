create macro m(x) as x + 1; create table t (id integer, a integer default m(1), b integer, check (b.m() > 0));
