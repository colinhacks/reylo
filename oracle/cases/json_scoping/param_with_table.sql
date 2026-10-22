with team as (select :n::int as name) select jsonb_build_object('n', team.name) as o from team
