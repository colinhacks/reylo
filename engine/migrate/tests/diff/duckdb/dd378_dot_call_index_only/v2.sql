create macro m(x) as x + 2; create table t (a integer); create index ti on t ((a.m()));
