create sequence seq increment by 3; create macro nid() as nextval('seq'); create table t (id bigint default nid(), n integer);
