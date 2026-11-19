create table p (a int, b int generated always as (a + 1) stored);
create table c (b int generated always as (a + 1) stored) inherits (p);
alter table c add constraint b_nn not null b not valid;
