with team as (select 'x'::text as motto) update team set name = name returning motto || '!' as m
