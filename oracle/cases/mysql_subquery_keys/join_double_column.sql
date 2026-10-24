select u.name from scores s join users u on u.id = s.score where s.id = :id
