create type b.mood as enum('a', 'b');
alter table b.p2 alter column d drop default;
drop type mood;
alter table b.p2 alter column d set default cast('a' as mood);
