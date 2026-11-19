-- loses data: t.s becomes a generated column, which replaces its values with the expression's
alter table t
  drop column g,
  drop column s,
  add column g int generated always as ((c + 1)) virtual,
  add column s int generated always as ((g + 1)) stored;
