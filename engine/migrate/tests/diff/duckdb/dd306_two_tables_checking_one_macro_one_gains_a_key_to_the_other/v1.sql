create macro valid(s) as s <> ''; create table a (id integer primary key, x varchar check (valid(x))); create table b (id integer primary key, a_id integer, y varchar check (valid(y)));
