with team as (select jsonb_build_object('k', 1) as motto) update team set name = name returning motto
