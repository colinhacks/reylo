create macro clamp (x) as (greatest(x, 0));
create table t (id bigint, n integer default clamp(-1), primary key (id), check ((clamp(n) = n)));
