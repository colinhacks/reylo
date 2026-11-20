create table t (a int, g int generated always as (a + 1) virtual);
