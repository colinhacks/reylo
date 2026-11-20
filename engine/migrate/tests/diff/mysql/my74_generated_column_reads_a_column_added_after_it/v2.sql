create table t (a int, g int generated always as (z + 1) virtual, z int);
