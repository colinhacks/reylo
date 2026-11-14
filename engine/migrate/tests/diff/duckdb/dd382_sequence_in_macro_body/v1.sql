create sequence seq; create macro nid() as nextval('seq'); create table t (id bigint default nid(), n integer);
