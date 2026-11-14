create macro bump(a) as a + 1; create table t (id integer primary key, n integer, m integer generated always as (bump(n)) virtual);
