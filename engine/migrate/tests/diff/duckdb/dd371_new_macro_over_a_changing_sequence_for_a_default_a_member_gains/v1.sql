create sequence s; create macro valid(c) as c <> ''; create table t (id bigint primary key default nextval('s'), code varchar check (valid(code)));
