with a as (select jsonb_build_object('n', tally.x) as o from tally), tally as (select 'one'::text as x) select o from a
