create schema s; create macro s.f(x) as x * 2; create table t (a integer); create index ti on t ((s.f(a)));
