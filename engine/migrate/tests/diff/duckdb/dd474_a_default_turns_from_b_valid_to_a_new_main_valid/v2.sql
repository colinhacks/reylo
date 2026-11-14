create schema b; create macro valid(s) as 'main'; create table b.p2 (id integer primary key, d varchar default main.valid('abc'));
