create table t (id int not null primary key);
GO
create schema billing;
GO
create table billing.invoices (id int not null primary key, t_id int not null references dbo.t (id), amount money not null);
