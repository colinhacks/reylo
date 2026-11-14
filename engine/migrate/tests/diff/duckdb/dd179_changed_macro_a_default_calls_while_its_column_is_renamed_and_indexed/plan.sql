alter table u alter column k drop default;
drop macro x;
create macro x () as (2);
alter table u alter column k set default x();
-- u.k is renamed kk, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table u rename column k to kk;
create index u_n on u(n);
