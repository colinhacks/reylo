create macro d(x) as x > 0; create table t (a integer, check (d(a)));
