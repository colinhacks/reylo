create macro m(x) as x > 0; create table t (a integer default 1, b integer, check (m(b)));
