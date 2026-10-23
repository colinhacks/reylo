update accounts set nickname = coalesce(:nickname::nullable(text), nickname), email = ifnull(:email::nullable(text), email) where id = :id returning nickname, email
