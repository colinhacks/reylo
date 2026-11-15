create macro m(x) as x > -1; create macro e(x) as x + 2; create table t (a integer default e(1), b integer, check (m(b)));
