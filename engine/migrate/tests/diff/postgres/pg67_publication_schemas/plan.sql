create schema audit;
create table audit.log (
  id integer not null,
  constraint log_pkey primary key (id)
);
alter publication pub set tables in schema app, tables in schema audit;
