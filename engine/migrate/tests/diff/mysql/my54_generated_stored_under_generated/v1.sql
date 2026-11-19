create table t (id int primary key, g int as (id + 1) virtual, h int as (g + 1) virtual);
