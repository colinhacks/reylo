create table users (id integer primary key, name text not null);
create schema billing;
create table billing.invoices (id integer primary key, user_id integer not null references users, total numeric not null);
create index invoices_user on billing.invoices (user_id);
