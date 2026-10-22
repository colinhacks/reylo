with team as (select 1 as motto) update team set name = name returning jsonb_build_object('m', motto) as j
