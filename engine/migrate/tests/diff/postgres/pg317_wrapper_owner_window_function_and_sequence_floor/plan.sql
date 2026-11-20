drop function rank_of(integer);
select setval('public.s', 10, false) from pg_sequences s where format('%I.%I', s.schemaname, s.sequencename)::regclass = 'public.s'::regclass and coalesce(s.last_value, s.start_value) < 10;
alter sequence s as bigint increment by 1 minvalue 10 maxvalue 9223372036854775807 start with 10 cache 1 no cycle;
create function rank_of(x int) returns int window language internal as 'window_rank';
alter default privileges grant select on large objects to diff_owner;
alter foreign data wrapper w owner to diff_super;
