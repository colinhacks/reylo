alter table t
  drop column g,
  drop column h,
  add column g int generated always as ((id + 1)) stored,
  add column h int generated always as ((g + 1)) virtual;
