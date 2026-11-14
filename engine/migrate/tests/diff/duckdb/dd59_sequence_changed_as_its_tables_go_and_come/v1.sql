create sequence s; create table t1 (id bigint default nextval('s'), a varchar); create table keep (id bigint default nextval('s'));
