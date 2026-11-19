alter table t remove partitioning;
alter table t
  drop primary key,
  add primary key (id);
