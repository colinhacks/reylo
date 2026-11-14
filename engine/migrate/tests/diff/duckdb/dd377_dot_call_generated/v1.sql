create macro m(x) as x + 1; create table t (a integer, b integer generated always as (a.m()));
