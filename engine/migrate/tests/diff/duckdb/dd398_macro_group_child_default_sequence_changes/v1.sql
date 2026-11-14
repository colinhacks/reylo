create sequence seq; create macro m(x) as x > 0; create table p (id integer primary key, check (m(id))); create table c (id bigint default nextval('seq'), p integer references p (id));
