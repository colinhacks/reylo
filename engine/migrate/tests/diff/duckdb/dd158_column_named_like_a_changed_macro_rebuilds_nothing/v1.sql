create macro score(x) as x * 2; create table t (id integer primary key, score integer, check (score >= 0));
