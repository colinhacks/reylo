create schema b; create macro valid(s) as 'main'; create table b.pages (id integer primary key, parent_id integer references b.pages (id), d varchar default valid('abc'));
