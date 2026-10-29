select u.name, t.title from users u left join teams t on t.id = u.team_id where t.title like 'a%'
