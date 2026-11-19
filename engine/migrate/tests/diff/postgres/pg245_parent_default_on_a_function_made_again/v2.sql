create function f(a text) returns text language sql immutable return '8';
create table p (id int, c smallint default f('7')::smallint);
create table h () inherits (p);
