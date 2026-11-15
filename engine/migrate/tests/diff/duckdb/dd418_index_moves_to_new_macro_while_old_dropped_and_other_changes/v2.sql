create macro n(x) as x + 3; create macro k(x) as x * 3; create table t (a integer); create index ti on t ((n(a))); create index tk on t ((k(a)));
