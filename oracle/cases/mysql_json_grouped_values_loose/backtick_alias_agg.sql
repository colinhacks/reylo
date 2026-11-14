select json_object('k', u.name, 'n', `count`) as o from (select count(*) as `count` from users) c, users u
