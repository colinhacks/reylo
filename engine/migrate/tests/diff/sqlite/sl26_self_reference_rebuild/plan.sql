create table new_people (id integer primary key, name text not null check (name <> ''), boss_id integer references people (id));
insert into new_people (id, name, boss_id) select id, name, boss_id from people;
drop table people;
alter table new_people rename to people;
