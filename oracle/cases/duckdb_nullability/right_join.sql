select u.name, t.title from users u right join teams t on t.id = u.team_id
