create sequence s; create macro nid() as nextval('s'); create macro ok(v) as v > 0; create table t (id bigint primary key default nid(), v integer, check (ok(v)));
