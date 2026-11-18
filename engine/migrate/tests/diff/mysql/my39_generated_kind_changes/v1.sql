create table readings (
  id int not null primary key,
  base int not null,
  doubled int generated always as (base * 2) stored,
  tripled int generated always as (base * 3) virtual,
  label varchar(20) generated always as ('x') virtual,
  plain int,
  later int
);
