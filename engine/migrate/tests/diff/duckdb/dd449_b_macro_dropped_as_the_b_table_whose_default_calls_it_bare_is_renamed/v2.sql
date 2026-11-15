create schema b; create macro valid(s) as 'main'; create table b.p3 (id integer primary key, d varchar default valid('abc'));
