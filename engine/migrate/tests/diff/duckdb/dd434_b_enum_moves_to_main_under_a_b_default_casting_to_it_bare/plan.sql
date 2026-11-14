create type mood as enum('a', 'c');
alter table b.p2 alter column m drop default;
drop type b.mood;
alter table b.p2 alter column m set default cast('a' as mood);
