-- loses data: readings.label stops being a virtual column, which leaves its rows without the values it computed
-- loses data: readings.later becomes a generated column, which replaces its values with the expression's
-- loses data: readings.plain becomes a generated column, which replaces its values with the expression's
alter table readings
  drop column doubled,
  drop column tripled,
  drop column label,
  drop column plain,
  add column doubled int generated always as ((base * 2)) virtual,
  add column tripled int generated always as ((base * 3)) stored,
  add column label varchar(20),
  add column plain int generated always as ((base + 1)) virtual,
  modify column later int generated always as ((base + 2)) stored;
