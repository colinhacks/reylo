create macro next_id() as (random() * 1e9)::bigint; create table t (id bigint primary key default next_id(), n integer);
