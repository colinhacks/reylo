insert into users (id, email, age) values (:id, :email, :age) on duplicate key update email = values(email)
