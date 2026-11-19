create table t (id int primary key, g int as (id + 1) stored, h int as (g + 1) virtual);
