create sequence s increment by 3; create table y (id integer primary key, xid integer, e bigint default nextval('s'));
