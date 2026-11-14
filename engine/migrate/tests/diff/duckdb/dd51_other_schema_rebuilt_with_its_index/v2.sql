create schema billing; create table billing.inv (id bigint primary key, total integer check (total >= 0)); create index inv_total on billing.inv (total);
