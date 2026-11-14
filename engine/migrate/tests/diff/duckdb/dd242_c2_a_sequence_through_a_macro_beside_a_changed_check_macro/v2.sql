create sequence s increment by 3; create macro nid() as nextval('s'); create macro ok(v) as v > 1; create table t (id bigint primary key default nid(), v integer, check (ok(v)));
