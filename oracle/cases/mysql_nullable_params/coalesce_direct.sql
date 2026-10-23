select id, name from users where coalesce(:email::nullable(text), email) = email or ifnull(:bio::nullable(text), 'x') = bio
