create schema b; create macro b.valid(s) as length(s) > 5; create macro valid(s) as s <> ''; create table b.p2 (id integer primary key, d boolean default valid('abc'));
