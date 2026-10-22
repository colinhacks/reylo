with tally as (select 'one'::text as x), a as (select jsonb_build_object('n', tally.x) as o from tally) select o from a
