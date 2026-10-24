select max(p.note) over (partition by p.user_id) as last_note from posts p where p.note is not null
