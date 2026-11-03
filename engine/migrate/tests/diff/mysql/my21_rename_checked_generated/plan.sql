-- users.n is renamed total, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users
  drop check n_pos,
  rename column n to total,
  modify column n2 int generated always as ((total * 2)) virtual,
  add constraint n_pos check ((total > 0));
