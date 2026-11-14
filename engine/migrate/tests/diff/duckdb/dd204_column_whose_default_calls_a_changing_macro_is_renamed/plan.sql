alter table t alter column code drop default;
drop macro m;
create macro m () as (2);
alter table t alter column code set default m();
-- t.code is renamed ref_code, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table t rename column code to ref_code;
