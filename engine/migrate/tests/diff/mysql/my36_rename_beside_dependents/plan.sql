-- users.a is renamed alpha, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users
  rename column a to alpha,
  add column tag_id int,
  add key tag_id (tag_id);
alter table users add constraint users_tag foreign key (tag_id) references tags (id);
create view tagged as select users.id as id, users.alpha as alpha, users.tag_id as tag_id from users;
