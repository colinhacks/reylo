select o.id, name, email, r.rating, exists (select 1 from orders x where x.user_id = o.user_id and x.id <> o.id) as repeat_buyer
from orders o
join users u on u.id = o.user_id
left join reviews r on r.user_id = u.id
