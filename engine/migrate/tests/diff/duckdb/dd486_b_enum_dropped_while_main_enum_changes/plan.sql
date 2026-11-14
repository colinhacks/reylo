drop type mood;
create type mood as enum('a', 'b', 'z');
alter table b.p2 alter column d drop default;
drop type b.mood;
alter table b.p2 alter column d set default cast('a' as mood);
