create schema b; create macro valid(s) as 'main'; create table b.p2 (id integer primary key, e varchar default valid('abc'));
