create table t1 (
  id integer not null,
  n0 text default '7',
  constraint t1_pkey primary key (id)
);
create table h0 (
  n0 text default '7' not null,
  constraint h0_pkey primary key (id)
) inherits (t1);
