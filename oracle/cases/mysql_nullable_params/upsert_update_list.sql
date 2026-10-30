insert into users (name, email) values (:name, :email) on duplicate key update bio = :bio, name = :again
