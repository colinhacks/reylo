create table t0 (
  id integer not null,
  constraint t0_pkey primary key (id)
) partition by range (id);
create table t0_pd partition of t0 default;
create table t2 (
  id integer not null,
  parent integer,
  constraint t2_pkey primary key (id)
);
alter table t2 add constraint t2_parent_fkey foreign key (parent) references t0 (id);
