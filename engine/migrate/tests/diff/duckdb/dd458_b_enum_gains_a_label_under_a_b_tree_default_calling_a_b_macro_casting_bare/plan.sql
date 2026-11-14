alter table b.pages alter column k drop default;
drop type b.mood;
create type b.mood as enum('b', 'c');
alter table b.pages alter column k set default b.cm();
