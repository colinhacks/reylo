alter table t alter column n drop default;
drop macro gen;
create macro gen () as (43);
alter table t alter column n set default gen();
