create macro bump(a, b) as a + b; create table t (id integer primary key, n integer, m integer generated always as (bump(n, 2)) virtual);
