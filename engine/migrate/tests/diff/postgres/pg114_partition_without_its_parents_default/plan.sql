create table p partition of t for values from (0) to (10);
alter table p alter column id drop default;
