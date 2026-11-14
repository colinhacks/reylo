create macro ok(x) as x >= 0; create table t (id integer, v integer check (v > 0));
