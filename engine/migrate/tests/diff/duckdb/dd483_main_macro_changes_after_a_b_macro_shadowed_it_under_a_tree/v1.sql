create schema b; create macro valid() as 1; create table b.pages (id integer primary key, parent_id integer references b.pages (id), d integer default valid());
