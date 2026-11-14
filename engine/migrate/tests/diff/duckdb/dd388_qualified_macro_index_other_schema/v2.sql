create schema s; create macro s.f(x) as x * 3; create table t (a integer); create index ti on t ((s.f(a)));
