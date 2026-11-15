create macro m(x) as x + 2; create table t (a integer, b integer generated always as (a.m()));
