alter table t
  drop check ck,
  add constraint ck check ((a > 0)) not enforced;
