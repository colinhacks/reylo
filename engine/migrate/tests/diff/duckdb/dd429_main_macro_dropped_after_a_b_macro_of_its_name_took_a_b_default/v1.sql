create schema b; create macro valid(s) as 'main'; create table b.t (id integer primary key, d varchar default valid('abc'));
