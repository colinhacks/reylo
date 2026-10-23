update accounts set nickname = coalesce(:nickname::nullable(varchar(20)), nickname), email = ifnull(:email::nullable(varchar(40)), email) where id = :id
