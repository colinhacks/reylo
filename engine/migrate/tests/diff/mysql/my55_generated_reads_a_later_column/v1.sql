create table t (id int primary key, c int, g int as (c + 1) stored, s int);
