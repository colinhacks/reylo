create table tokens (
  id binary(16) not null default (unhex(replace(uuid(), _utf8mb4'-', _utf8mb4''))),
  primary key (id)
);
alter table users
  modify column name text not null default (cast(_utf8mb4'foo' as char charset ascii)),
  add column data blob default (cast(_utf8mb4'foo' as char charset binary));
