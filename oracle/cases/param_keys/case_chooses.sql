select id, name from users order by case :sort::text when 'name' then name end, id
