alter table t
  drop check ck,
  drop column g,
  add column g int generated always as ((c + 1)) stored,
  add constraint ck check ((g > c));
