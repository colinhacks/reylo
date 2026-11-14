create macro clamp(x) as greatest(x, 0); create table t (id bigint primary key, n integer default clamp(-1) check (clamp(n) = n));
