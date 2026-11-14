create type mood as enum('a', 'c');
alter table b.pages alter column m set default cast('a' as main.mood);
drop type b.mood;
