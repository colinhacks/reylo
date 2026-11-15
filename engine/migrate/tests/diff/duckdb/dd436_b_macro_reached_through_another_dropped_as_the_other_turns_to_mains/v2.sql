create schema b; create macro valid(s) as 'main'; create macro b.wrap() as main.valid('x'); create table b.p2 (id integer primary key, d varchar default b.wrap());
