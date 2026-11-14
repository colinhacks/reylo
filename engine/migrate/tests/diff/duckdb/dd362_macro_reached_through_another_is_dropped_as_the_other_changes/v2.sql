create macro outer_m() as 2; create table t (id integer primary key, n integer check (n < outer_m() + 10));
