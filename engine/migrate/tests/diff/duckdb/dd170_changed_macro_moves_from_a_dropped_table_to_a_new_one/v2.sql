create macro ok(a) as a >= 0; create table u (id integer primary key, v integer, check (ok(v)));
