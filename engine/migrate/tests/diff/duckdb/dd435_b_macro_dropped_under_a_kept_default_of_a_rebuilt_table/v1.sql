create schema b; create macro b.valid(s) as 'b'; create macro valid(s) as 'main'; create table b.p2 (id integer primary key, n integer, d varchar default valid('abc'));
