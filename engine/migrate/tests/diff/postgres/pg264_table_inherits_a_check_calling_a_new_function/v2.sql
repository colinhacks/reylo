create table p (id int);
create table t (id int) inherits (p);
create table h (e text);
create function r() returns text language sql stable begin atomic select min(e) from h; end;
alter table p add constraint ck check (r() = r());
