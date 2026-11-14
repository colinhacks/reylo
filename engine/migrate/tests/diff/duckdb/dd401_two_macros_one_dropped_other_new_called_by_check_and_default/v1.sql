create macro m(x) as x > 0; create macro d(x) as x + 1; create table t (a integer default d(1), b integer, check (m(b)));
