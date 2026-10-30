select +name as p, -age as m from users where name in (:n) and id in (:i)
