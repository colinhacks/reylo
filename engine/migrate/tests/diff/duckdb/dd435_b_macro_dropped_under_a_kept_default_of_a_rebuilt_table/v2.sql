create schema b; create macro valid(s) as 'main'; create table b.p2 (id integer primary key, n integer check (n > 0), d varchar default valid('abc'));
