create table t (id int primary key, c int, g int as (c + 1) virtual, s int as (g + 1) stored);
