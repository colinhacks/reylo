create schema b; create macro valid(s) as s <> ''; create table b.p2 (id integer primary key, d boolean default valid('abc'));
