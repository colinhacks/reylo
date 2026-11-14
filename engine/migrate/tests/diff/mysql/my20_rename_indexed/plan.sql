-- users.email is renamed mail, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users
  rename column email to mail,
  add column extra int;
