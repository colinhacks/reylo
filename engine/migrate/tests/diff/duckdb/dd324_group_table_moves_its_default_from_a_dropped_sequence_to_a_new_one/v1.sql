create sequence old_s; create macro valid(c) as c <> ''; create table t (id bigint primary key default nextval('old_s'), code varchar check (valid(code)));
