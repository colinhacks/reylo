create schema b; create macro valid(s) as 'main2'; create macro b.valid(s) as 'b'; create table b.t (id integer primary key, d varchar default valid('abc'));
