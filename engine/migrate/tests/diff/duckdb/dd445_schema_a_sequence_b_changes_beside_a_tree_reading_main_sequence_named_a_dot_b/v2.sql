create schema a; create sequence a.b increment by 4; create sequence "a.b"; create table pages (id integer primary key, parent_id integer references pages (id), k bigint default nextval('"a.b"'));
