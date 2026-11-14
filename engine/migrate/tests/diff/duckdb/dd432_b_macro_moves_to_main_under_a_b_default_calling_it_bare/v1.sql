create schema b; create macro b.valid(s) as 'b'; create table b.p2 (id integer primary key, d varchar default valid('abc'));
