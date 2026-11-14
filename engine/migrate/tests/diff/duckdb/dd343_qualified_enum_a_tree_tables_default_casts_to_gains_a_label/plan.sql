alter table pages alter column m drop default;
drop type b.mood;
create type b.mood as enum('a', 'b', 'c');
alter table pages alter column m set default cast('a' as b.mood);
