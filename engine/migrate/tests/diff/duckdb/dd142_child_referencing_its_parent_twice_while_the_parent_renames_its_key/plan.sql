create temporary table reylo_copy_person as select * from person;
create temporary table reylo_copy_friendship as select * from friendship;
drop table friendship;
drop table person;
create table person (pid integer, name varchar, primary key (pid), check (("name" != '')));
create table friendship (a integer, b integer, since date, foreign key (a) references person (pid), foreign key (b) references person (pid));
insert into person (pid, name) select id, name from temp.main.reylo_copy_person;
insert into friendship (a, b, since) select a, b, since from temp.main.reylo_copy_friendship;
drop table temp.main.reylo_copy_person;
drop table temp.main.reylo_copy_friendship;
-- person.id is renamed pid, keeping its rows. If they are two columns, drop the one and add the other instead.
