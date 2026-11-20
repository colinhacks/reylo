-- loses data: dog
drop table dog;
create table hyena (
  id integer not null,
  name text not null,
  constraint hyena_pkey primary key (id)
);
-- age is not null with no default, so adding it fails on a table that has rows: give it a default, or add it nullable, fill it, then set not null
alter table cat add column age integer not null;
