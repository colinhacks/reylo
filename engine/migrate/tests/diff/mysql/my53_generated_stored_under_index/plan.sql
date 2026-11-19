alter table t
  drop index k,
  drop column g,
  add column g int generated always as ((c + 1)) stored,
  add key k (((g * 2)));
