alter table t alter column id drop default;
drop macro next_id;
create macro next_id () as (cast((random() * 1e9) as BIGINT));
alter table t alter column id set default next_id();
alter table t alter column id drop default;
drop sequence s;
alter table t alter column id set default next_id();
