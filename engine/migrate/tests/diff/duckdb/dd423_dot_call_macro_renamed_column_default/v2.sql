create macro m(x) as x + 1; create table t (a integer default m(1), b integer, check (b.m() > 0)); create table u (z integer, y integer default m(3));
