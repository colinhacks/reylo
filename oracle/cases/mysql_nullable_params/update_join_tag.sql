update users u join posts p on p.user_id = u.id set u.email = :email, p.title = :title, p.tag = :tag where p.id = :id
