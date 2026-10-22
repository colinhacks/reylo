select json_build_object('score', t.score, 'rating', m.rating) as j from teams t join members m on m.team_id = t.id
