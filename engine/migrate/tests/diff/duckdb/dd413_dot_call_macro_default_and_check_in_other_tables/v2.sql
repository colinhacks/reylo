create macro m(x) as x + 2; create table t (a integer, check (a.m() > 0)); create table u (b integer default m(1));
