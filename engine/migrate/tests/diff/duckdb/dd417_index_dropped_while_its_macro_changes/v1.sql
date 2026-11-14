create macro m(x) as x + 1; create table t (a integer); create index ti on t ((m(a)));
