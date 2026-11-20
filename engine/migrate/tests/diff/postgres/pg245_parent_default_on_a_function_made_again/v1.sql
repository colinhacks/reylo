create function f() returns text language sql immutable return '8';
create table p (id int, c smallint default f()::smallint);
create table h () inherits (p);
