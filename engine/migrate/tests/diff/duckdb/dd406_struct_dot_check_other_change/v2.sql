create table t (meta struct(a integer), n integer, m integer, check (meta.a > 0)); create table u (x integer, check (x > 0 and x < 20));
