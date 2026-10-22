select t.id, json_agg(m.*) filter (where m.id is not null) as members from teams t left join members m on m.team_id = t.id group by t.id
