create sequence zs; create macro a_lead(s) as s <> ''; create table t (id bigint primary key default nextval('zs'), code varchar check (a_lead(code)));
