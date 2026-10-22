select (with team as (select 1 as name) select jsonb_agg(team.name) from team) as v
