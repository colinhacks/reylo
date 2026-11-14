create schema b; create macro b.valid(s) as 'b'; create macro valid(s) as 'main'; create macro b.wrap() as b.valid('x'); create table b.p2 (id integer primary key, d varchar default b.wrap());
