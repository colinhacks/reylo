select u.name, t.title, t.budget from users u left join teams t on t.id = u.team_id
