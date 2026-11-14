alter table pages alter column a drop default;
drop type mood;
create type mood as enum('a', 'b', 'c');
alter table pages alter column a set default cast(cast('a' as mood) as VARCHAR);
