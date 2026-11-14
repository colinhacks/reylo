create type mood as enum ('a', 'b', 'c'); create macro ok(x) as x::mood is not null; create table t (a varchar, check (ok(a)));
