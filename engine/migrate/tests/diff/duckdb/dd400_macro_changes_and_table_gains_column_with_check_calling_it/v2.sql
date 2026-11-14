create macro m(x) as x > -1; create table t (a integer default 1, b integer, c integer default 3, check (m(b)), check (m(c)));
