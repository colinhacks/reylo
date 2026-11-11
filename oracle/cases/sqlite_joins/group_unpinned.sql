select title, count(*) as n from posts where user_id = :u group by title
