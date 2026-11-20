alter table t
  modify column g int generated always as ((z + 1)) virtual,
  add column z int;
