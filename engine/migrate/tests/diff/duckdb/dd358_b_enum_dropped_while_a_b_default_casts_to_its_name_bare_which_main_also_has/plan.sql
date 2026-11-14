alter table b.t alter column m drop default;
drop type b.mood;
alter table b.t alter column m set default cast('a' as mood);
