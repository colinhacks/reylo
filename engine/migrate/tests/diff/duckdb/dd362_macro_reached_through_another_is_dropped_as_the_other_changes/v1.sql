create macro inner_m() as 1; create macro outer_m() as inner_m() + 1; create table t (id integer primary key, n integer check (n < outer_m() + 10));
