create schema billing;
GO
create table billing.invoices (
  id int not null,
  t_id int not null,
  amount money not null,
  primary key clustered (id)
);
alter table billing.invoices add foreign key (t_id) references t (id);
