create sequence new_s start 100; create macro valid(c) as length(c) > 0; create table t (id bigint primary key default nextval('new_s'), code varchar check (valid(code)));
