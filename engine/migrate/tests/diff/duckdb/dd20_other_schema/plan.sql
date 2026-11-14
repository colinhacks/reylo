alter table billing.invoices alter column total set data type decimal(12,2);
alter table billing.invoices add column paid boolean default false;
create table billing.items (id bigint, invoice_id bigint, primary key (id), foreign key (invoice_id) references invoices (id));
