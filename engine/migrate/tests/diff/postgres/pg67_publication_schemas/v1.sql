create schema app;
create table app.t (id integer primary key);
create publication pub for tables in schema app;
