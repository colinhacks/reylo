select id, name from users order by case when :desc::bool then created_at end desc, id
