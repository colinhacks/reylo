create macro score(x) as x * 3; create table t (id integer primary key, score integer, check (score >= 0));
