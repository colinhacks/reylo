create sequence s; create table x (id integer primary key, d bigint default nextval('s')); create table y (id integer primary key, xid integer references x (id), e bigint default nextval('s'));
