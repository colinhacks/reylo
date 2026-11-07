-- users.a is renamed alpha, keeping its rows. If they are two columns, drop the one and add the other instead.
-- users.b is renamed beta, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users
  rename column a to alpha,
  rename column b to beta,
  add key beta_idx (beta);
