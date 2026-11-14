create macro m(x) as x + 1; create macro k(x) as x * 2; create table t (a integer); create index ti on t ((m(a))); create index tk on t ((k(a)));
