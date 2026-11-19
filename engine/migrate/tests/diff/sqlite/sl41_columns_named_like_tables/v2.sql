create table a (id integer primary key, b text, n integer not null default 0 check (n >= 0));
create table b (id integer primary key, a text);
