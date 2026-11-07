create schema app;
create schema audit;
create table app.t (id integer primary key);
create table audit.log (id integer primary key);
create publication pub for tables in schema app, audit;
