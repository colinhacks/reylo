select p.user_id, max(p.note) as last_note from posts p where p.note is not null group by p.user_id
