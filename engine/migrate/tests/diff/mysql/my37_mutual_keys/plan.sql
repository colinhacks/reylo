-- loses data: a
-- loses data: b
alter table a drop foreign key a_b, drop index a_b;
alter table b drop foreign key b_a, drop index b_a;
drop table a;
drop table b;
create table c (
  id int not null,
  primary key (id)
);
