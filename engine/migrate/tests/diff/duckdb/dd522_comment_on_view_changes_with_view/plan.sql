drop view recent_titles;
drop view recent;
create view recent as select p.id, p.title, u."name", p.body from (posts as p inner join users as u on ((u.id = p.user_id)));
create view recent_titles as select title from recent;
comment on view recent is 'latest posts';
