select id, name from users order by case :sort when 'name' then name end, id
