delete from users where id = abs(random()) % 3 returning id
