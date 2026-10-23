update accounts set nickname = coalesce(:nickname::nullable(text), nickname), email = coalesce(:email::nullable(text), email) where id = :id returning nickname, email
