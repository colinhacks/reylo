select u.id as user_id, t.id as team_id from users u full join teams t on t.id = u.team_id
