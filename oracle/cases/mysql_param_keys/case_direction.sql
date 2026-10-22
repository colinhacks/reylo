select id, name from users order by case when :desc then created_at end desc, id
