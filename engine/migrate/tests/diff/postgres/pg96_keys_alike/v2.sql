create table kept (id integer primary key);
create table t (a integer, unique (a));
alter table t add unique (a);
create table company_integration (id integer, company_id integer, constraint company_integration_pkey primary key (id, company_id), constraint company_integration_id_company_id_unique unique (id, company_id));
