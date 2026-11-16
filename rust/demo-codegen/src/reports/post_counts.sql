select u.name, count(p.id) as posts, max(u.joined) as joined
from users u left join posts p on p.author_id = u.id
group by u.name order by u.name
