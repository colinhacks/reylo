create table p (id int, c int, g int generated always as (c * 2) stored);
create table h ();
