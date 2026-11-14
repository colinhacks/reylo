create type mood as enum ('a', 'b'); create macro ok(x mood) as x is not null; create table t (a varchar default 'a', b mood default 'b', check (ok(a::mood)));
