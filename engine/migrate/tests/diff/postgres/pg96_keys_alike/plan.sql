create table company_integration (
  id integer not null,
  company_id integer not null,
  constraint company_integration_pkey primary key (id, company_id)
);
create table t (
  a integer,
  constraint t_a_key unique (a)
);
alter table t add constraint t_a_key1 unique (a);
