create table readings (
  id int not null primary key,
  base int not null,
  doubled int generated always as (base * 2) virtual,
  tripled int generated always as (base * 3) stored,
  label varchar(20),
  plain int generated always as (base + 1) virtual,
  later int generated always as (base + 2) stored
);
