create macro x(a) as a > 0; create macro x(a) as table select a as v; create table pages (id integer primary key, parent_id integer references pages (id), n integer check (x(n)));
