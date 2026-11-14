select (select json_object('n', count(*), 's', (select json_object('c', count(*), 'k', max(X.id)) from posts p)) from users X where false) as o
