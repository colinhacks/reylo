create schema billing;
create table billing.invoices (
  id integer not null,
  user_id integer not null,
  total numeric not null,
  constraint invoices_pkey primary key (id)
);
alter table billing.invoices add constraint invoices_user_id_fkey foreign key (user_id) references users(id);
create index invoices_user on billing.invoices (user_id);
