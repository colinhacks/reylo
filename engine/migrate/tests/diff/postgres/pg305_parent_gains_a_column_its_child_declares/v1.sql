create table t1 (
  id integer not null,
  constraint t1_pkey primary key (id)
);
create table h0 (
  constraint h0_pkey primary key (id)
) inherits (t1);
