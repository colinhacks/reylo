create table p (a int, b int generated always as (a + 1) stored);
create table c (b int generated always as (a + 2) stored) inherits (p);
